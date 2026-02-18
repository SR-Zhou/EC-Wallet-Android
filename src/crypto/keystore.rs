use ark_ec::CurveGroup;
use ark_ec::PrimeGroup;
use ark_ff::BigInteger;
use ark_ff::Field;
use ark_ff::PrimeField;
use ark_secp256k1::Fq;
use ark_secp256k1::Fr;
use ark_secp256k1::Affine;
use ark_secp256k1::Projective;
use sha3::Digest;
use sha3::Keccak256;
use std::fs;
use std::str::FromStr;
use super::elgamal;

// x 是否在曲线上
fn x_is_on_curve(x: &Fq) -> bool
{
    let y = x * x * x + Fq::from(7u8);
    y.sqrt().is_some()
}

// 私钥编码为曲线上的点，返回点和可选的 padding
fn encode(sk: String) -> Result<(Projective, Option<u8>), String>
{
    let s = Fq::from_be_bytes_mod_order(&hex::decode(sk.trim()).expect("Invalid hex."));

    let (x, padding) = if !x_is_on_curve(&s)
    {
        let mut padding = 0u8;
        let mut found = false;
        for _ in 0..255
        {
            padding += 1;
            let new_x = s + Fq::from(padding);
            if x_is_on_curve(&new_x)
            {
                found = true;
                break;
            }
        }
        if !found
        {
            return Err("Failed to find a valid padding.".to_string());
        }
        (s + Fq::from(padding), Some(padding))
    }
    else
    {
        (s, None)
    };
    
    let y_squared = x * x * x + Fq::from(7u8);
    let y = y_squared.sqrt().unwrap();
    let y = if y.into_bigint().is_odd() { y } else { -y };
    let ret = Projective::from(Affine::new(x, y));

    Ok((ret, padding))
}

// 解码曲线上的点，提取 x 坐标
fn decode(point: Projective) -> Fq
{
    let affinepoint = point.into_affine();
    affinepoint.x
}

fn f(s: String) -> Result<Fr, String>
{
    if s.trim().is_empty()
    {
        return Err("Input string is empty".to_string());
    }
    let mut result = Fr::from(1u64);
    for num_str in s.split_whitespace()
    {
        let num = Fr::from_str(num_str)
            .map_err(|_| format!("Failed to parse '{}' as a number", num_str))?;
        result *= Fr::from(num);
    }
    Ok(result)
}

fn g(u: Fr) -> Fr
{
    let mut result = u;
    for _ in 1..9
    {
        result *= u;
    }
    result
}

// API
// 使用口令派生密钥加密私钥并存储到文件
pub fn store(password: String, sk: String, path: &std::path::Path, flag: bool) -> Result<(), String>
{
    let ((c1, c2), padding) = match flag
    {
        true =>
        {
            let result = f(password)?;
            let result = g(result);
            let result = Projective::generator() * result;
            let (m, padding) = encode(sk)?;
            (elgamal::elgamal_encrypt(result, m), padding)
        },
        false =>
        {
            let mut hasher = Keccak256::new();
            hasher.update(password.trim().as_bytes());
            let result = hasher.finalize();
            let result = Fr::from_be_bytes_mod_order(result.as_slice());
            let result = Projective::generator() * result;
            let (m, padding) = encode(sk)?;
            (elgamal::elgamal_encrypt(result, m), padding)
        },
    };
    
    // 确保父目录存在
    if let Some(parent) = path.parent()
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    
    let data = serialize_points(&c1, &c2, padding);
    fs::write(path, data)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(())
}

// API
// 使用口令派生密钥解密存储的私钥
pub fn load(password: String, path: &std::path::Path, flag: bool) -> Result<String, String>
{
    if !path.exists()
    {
        return Err(format!("File '{}' not found", path.display()));
    }
    let data = fs::read(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let ((c1, c2), padding) = deserialize_points(&data)?;
    let sk = match flag
    {
        true =>
        {
            let result = f(password)?;
            let result = g(result);
            let decrypted_point = elgamal::elgamal_decrypt(result, c1, c2);
            decode(decrypted_point)
        },
        false =>
        {
            let mut hasher = Keccak256::new();
            hasher.update(password.as_bytes());
            let result = hasher.finalize();
            let result = Fr::from_be_bytes_mod_order(result.as_slice());
            let decrypted_point = elgamal::elgamal_decrypt(result, c1, c2);
            decode(decrypted_point)
        },
    };
    
    // 如果有 padding，减去 padding 恢复原始 sk
    if let Some(pad) = padding
    {
        return Ok(hex::encode((sk - Fq::from(pad)).into_bigint().to_bytes_be()));
    }

    Ok(hex::encode(sk.into_bigint().to_bytes_be()))
}

fn serialize_points(c1: &Projective, c2: &Projective, padding: Option<u8>) -> Vec<u8>
{
    let c1_affine = c1.into_affine();
    let c2_affine = c2.into_affine();
    
    let mut data = Vec::new();

    data.extend_from_slice(&c1_affine.x.into_bigint().to_bytes_be());
    data.extend_from_slice(&c1_affine.y.into_bigint().to_bytes_be());
    
    data.extend_from_slice(&c2_affine.x.into_bigint().to_bytes_be());
    data.extend_from_slice(&c2_affine.y.into_bigint().to_bytes_be());
    
    // 如果有 padding，追加到末尾
    if let Some(pad) = padding
    {
        data.push(pad);
    }
    
    data
}

fn deserialize_points(data: &[u8]) -> Result<((Projective, Projective), Option<u8>), String>
{
    // 128 字节(无 padding)或 129 字节(有 padding)
    let has_padding = match data.len()
    {
        128 => false,
        129 => true,
        _ => return Err(format!("Invalid data length: expected 128 or 129 bytes, got {}", data.len())),
    };
    
    let c1_bytes = &data[0..64];
    let c2_bytes = &data[64..128];
    let padding = if has_padding { Some(data[128]) } else { None };
    
    // 解析 c1
    let c1_x_bytes: [u8; 32] = c1_bytes[0..32].try_into()
        .map_err(|_| "Failed to parse c1 x coordinate".to_string())?;
    let c1_y_bytes: [u8; 32] = c1_bytes[32..64].try_into()
        .map_err(|_| "Failed to parse c1 y coordinate".to_string())?;
    
    let c1_x = Fq::from_be_bytes_mod_order(&c1_x_bytes);
    let c1_y = Fq::from_be_bytes_mod_order(&c1_y_bytes);
    
    // 解析 c2
    let c2_x_bytes: [u8; 32] = c2_bytes[0..32].try_into()
        .map_err(|_| "Failed to parse c2 x coordinate".to_string())?;
    let c2_y_bytes: [u8; 32] = c2_bytes[32..64].try_into()
        .map_err(|_| "Failed to parse c2 y coordinate".to_string())?;
    
    let c2_x = Fq::from_be_bytes_mod_order(&c2_x_bytes);
    let c2_y = Fq::from_be_bytes_mod_order(&c2_y_bytes);
    
    // 构造仿射点
    let c1_affine = Affine::new(c1_x, c1_y);
    let c2_affine = Affine::new(c2_x, c2_y);
    
    // 验证点是否在曲线上
    if !c1_affine.is_on_curve() || !c2_affine.is_on_curve()
    {
        return Err("Decoded points are not on the curve".to_string());
    }
    
    // 转换为投影点
    let c1_proj = Projective::from(c1_affine);
    let c2_proj = Projective::from(c2_affine);
    
    Ok(((c1_proj, c2_proj), padding))
}