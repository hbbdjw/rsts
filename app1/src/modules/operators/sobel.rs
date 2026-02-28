use super::Operator;
use anyhow::{Context, Result};
use async_trait::async_trait;
use image::{DynamicImage, GrayImage, ImageReader};
use rayon::prelude::*;
use std::fmt::Debug;
use std::io::Cursor;
use std::path::PathBuf;

/// Sobel算子：用于边缘检测
///
/// 算法原理：
/// Sobel算子通过计算图像灰度的近似梯度来检测边缘。
/// 它使用两个3x3的卷积核分别计算水平方向(Gx)和垂直方向(Gy)的梯度。
///
/// Gx = [[-1, 0, 1],
///       [-2, 0, 2],
///       [-1, 0, 1]]
///
/// Gy = [[-1, -2, -1],
///       [ 0,  0,  0],
///       [ 1,  2,  1]]
///
/// 最终梯度幅值 G = sqrt(Gx² + Gy²)
#[derive(Debug, Clone)]
pub struct SobelOperator {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

impl SobelOperator {
    pub fn new(input_path: impl Into<PathBuf>, output_path: impl Into<PathBuf>) -> Self {
        Self {
            input_path: input_path.into(),
            output_path: output_path.into(),
        }
    }

    /// 执行Sobel边缘检测算法
    fn apply_sobel(&self) -> Result<()> {
        // 1. 读取图像
        let img = ImageReader::open(&self.input_path)
            .with_context(|| format!("无法打开输入图像: {:?}", self.input_path))?
            .decode()
            .context("无法解码图像")?;

        // 2. 处理图像
        let output_img = Self::compute_sobel(&img.to_luma8());

        // 3. 保存结果
        output_img
            .save(&self.output_path)
            .with_context(|| format!("无法保存输出图像: {:?}", self.output_path))?;

        Ok(())
    }

    /// 对缓冲区中的图像数据应用Sobel算子
    pub fn apply_to_buffer(input_data: &[u8]) -> Result<Vec<u8>> {
        // 1. 解码图像
        let img = ImageReader::new(Cursor::new(input_data))
            .with_guessed_format()?
            .decode()
            .context("无法解码图像数据")?;

        // 2. 处理图像
        let output_img = Self::compute_sobel(&img.to_luma8());

        // 3. 编码结果为PNG
        let mut buffer = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(output_img).write_to(&mut buffer, image::ImageFormat::Png)?;

        Ok(buffer.into_inner())
    }

    /// Sobel核心计算逻辑
    #[cfg(feature = "simd")]
    fn compute_sobel(gray_img: &GrayImage) -> GrayImage {
        Self::compute_sobel_simd(gray_img)
    }

    #[cfg(not(feature = "simd"))]
    fn compute_sobel(gray_img: &GrayImage) -> GrayImage {
        Self::compute_sobel_parallel_scalar(gray_img)
    }

    fn compute_sobel_parallel_scalar(gray_img: &GrayImage) -> GrayImage {
        let (width_u32, height_u32) = gray_img.dimensions();
        let width = width_u32 as usize;
        let height = height_u32 as usize;

        let input = gray_img.as_raw().as_slice();

        let mut output = vec![0u8; width.saturating_mul(height)];

        if width < 3 || height < 3 {
            return GrayImage::from_raw(width_u32, height_u32, output)
                .expect("invalid image dimensions");
        }

        let stride = width;
        let middle_start = stride;
        let middle_end = (height - 1) * stride;
        let middle = &mut output[middle_start..middle_end];

        middle
            .par_chunks_mut(stride)
            .enumerate()
            .for_each(|(row_idx, out_row)| {
                let y = row_idx + 1;

                unsafe {
                    let out_ptr = out_row.as_mut_ptr();
                    let row_prev = (y - 1) * stride;
                    let row_cur = y * stride;
                    let row_next = (y + 1) * stride;

                    for x in 1..width - 1 {
                        let p00 = *input.get_unchecked(row_prev + x - 1) as i32;
                        let p01 = *input.get_unchecked(row_prev + x) as i32;
                        let p02 = *input.get_unchecked(row_prev + x + 1) as i32;

                        let p10 = *input.get_unchecked(row_cur + x - 1) as i32;
                        let p12 = *input.get_unchecked(row_cur + x + 1) as i32;

                        let p20 = *input.get_unchecked(row_next + x - 1) as i32;
                        let p21 = *input.get_unchecked(row_next + x) as i32;
                        let p22 = *input.get_unchecked(row_next + x + 1) as i32;

                        let gx = (p02 + 2 * p12 + p22) - (p00 + 2 * p10 + p20);
                        let gy = (p20 + 2 * p21 + p22) - (p00 + 2 * p01 + p02);
                        let g = ((gx * gx + gy * gy) as f32).sqrt();
                        *out_ptr.add(x) = g.min(255.0) as u8;
                    }
                }
            });

        GrayImage::from_raw(width_u32, height_u32, output).expect("invalid image dimensions")
    }

    #[cfg(feature = "simd")]
    fn compute_sobel_simd(gray_img: &GrayImage) -> GrayImage {
        use std::simd::num::SimdUint;
        use std::simd::{Simd, SimdFloat};

        let (width_u32, height_u32) = gray_img.dimensions();
        let width = width_u32 as usize;
        let height = height_u32 as usize;

        let input = gray_img.as_raw();
        let in_ptr = input.as_ptr();

        let mut output = vec![0u8; width.saturating_mul(height)];

        if width < 3 || height < 3 {
            return GrayImage::from_raw(width_u32, height_u32, output)
                .expect("invalid image dimensions");
        }

        type U8x16 = Simd<u8, 16>;
        type I16x16 = Simd<i16, 16>;
        type I32x16 = Simd<i32, 16>;
        type F32x16 = Simd<f32, 16>;

        #[inline]
        unsafe fn load_u8x16(ptr: *const u8) -> U8x16 {
            let mut tmp = [0u8; 16];
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, tmp.as_mut_ptr(), 16);
            }
            U8x16::from_array(tmp)
        }

        #[inline]
        unsafe fn store_u8x16(ptr: *mut u8, v: U8x16) {
            let tmp = v.to_array();
            unsafe {
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), ptr, 16);
            }
        }

        let stride = width;
        let middle_start = stride;
        let middle_end = (height - 1) * stride;
        let middle = &mut output[middle_start..middle_end];

        middle
            .par_chunks_mut(stride)
            .enumerate()
            .for_each(|(row_idx, out_row)| {
                let y = row_idx + 1;

                unsafe {
                    let row_prev = in_ptr.add((y - 1) * stride);
                    let row_cur = in_ptr.add(y * stride);
                    let row_next = in_ptr.add((y + 1) * stride);
                    let out_ptr = out_row.as_mut_ptr();

                    let two_i16 = I16x16::splat(2);
                    let max_255 = F32x16::splat(255.0);

                    let mut x = 1usize;
                    let simd_end = width.saturating_sub(17);

                    while x <= simd_end {
                        let p00 = load_u8x16(row_prev.add(x - 1)).cast::<i16>();
                        let p01 = load_u8x16(row_prev.add(x)).cast::<i16>();
                        let p02 = load_u8x16(row_prev.add(x + 1)).cast::<i16>();

                        let p10 = load_u8x16(row_cur.add(x - 1)).cast::<i16>();
                        let p12 = load_u8x16(row_cur.add(x + 1)).cast::<i16>();

                        let p20 = load_u8x16(row_next.add(x - 1)).cast::<i16>();
                        let p21 = load_u8x16(row_next.add(x)).cast::<i16>();
                        let p22 = load_u8x16(row_next.add(x + 1)).cast::<i16>();

                        let gx: I16x16 = (p02 + p12 * two_i16 + p22) - (p00 + p10 * two_i16 + p20);
                        let gy: I16x16 = (p20 + p21 * two_i16 + p22) - (p00 + p01 * two_i16 + p02);

                        let gx_i32: I32x16 = gx.cast::<i32>();
                        let gy_i32: I32x16 = gy.cast::<i32>();
                        let sum: I32x16 = gx_i32 * gx_i32 + gy_i32 * gy_i32;

                        let mag: F32x16 = sum.cast::<f32>().sqrt();
                        let mag = mag.simd_min(max_255);
                        let out: U8x16 = mag.cast::<u8>();

                        store_u8x16(out_ptr.add(x), out);
                        x += 16;
                    }

                    while x < width - 1 {
                        let p00 = *row_prev.add(x - 1) as i32;
                        let p01 = *row_prev.add(x) as i32;
                        let p02 = *row_prev.add(x + 1) as i32;

                        let p10 = *row_cur.add(x - 1) as i32;
                        let p12 = *row_cur.add(x + 1) as i32;

                        let p20 = *row_next.add(x - 1) as i32;
                        let p21 = *row_next.add(x) as i32;
                        let p22 = *row_next.add(x + 1) as i32;

                        let gx = (p02 + 2 * p12 + p22) - (p00 + 2 * p10 + p20);
                        let gy = (p20 + 2 * p21 + p22) - (p00 + 2 * p01 + p02);
                        let g = ((gx * gx + gy * gy) as f32).sqrt();
                        *out_ptr.add(x) = g.min(255.0) as u8;

                        x += 1;
                    }
                }
            });

        GrayImage::from_raw(width_u32, height_u32, output).expect("invalid image dimensions")
    }
}

#[async_trait]
impl Operator for SobelOperator {
    async fn execute(&self) -> Result<(), anyhow::Error> {
        // 图像处理通常是CPU密集型任务，使用spawn_blocking避免阻塞异步运行时
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.apply_sobel())
            .await
            .context("任务执行被取消")??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_sobel_operator() -> Result<()> {
        let test_input = r"F:\tup\photo_2022-12-08_18-31-01.jpg";
        // 先输出到当前目录，避免可能的权限或路径问题
        let temp_output = "sobel_output.png";
        let target_output = r"F:\tup\sobel_output_photo_2022-12-08_18-31-01.png";

        // 检查输入文件是否存在
        if !fs::metadata(test_input).is_ok() {
            println!("测试文件不存在，跳过测试: {}", test_input);
            return Ok(());
        }

        println!("开始处理图像...");
        let operator = SobelOperator::new(test_input, temp_output);
        operator.execute().await?;

        assert!(fs::metadata(temp_output).is_ok());
        println!("Sobel算子处理完成，临时输出文件: {}", temp_output);

        // 尝试移动到目标目录
        match fs::copy(temp_output, target_output) {
            Ok(_) => {
                println!("成功保存到目标路径: {}", target_output);
                // 删除临时文件
                let _ = fs::remove_file(temp_output);
            }
            Err(e) => {
                println!("无法保存到目标路径 ({}): {}", target_output, e);
                println!("结果保留在: {}", temp_output);
            }
        }

        Ok(())
    }
}
