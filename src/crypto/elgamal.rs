use ark_ec::PrimeGroup;
use ark_ff::PrimeField;
use ark_secp256k1::Fr;
use ark_secp256k1::Projective;

pub(crate) fn elgamal_encrypt(pk: Projective, m: Projective) -> (Projective, Projective)
{
    let k = Fr::from_be_bytes_mod_order(&rand::random::<[u8; 32]>());
    let c1 = Projective::generator() * k;
    let c2 = m + pk * k;
    (c1, c2)
}

pub(crate) fn elgamal_decrypt(sk: Fr, c1: Projective, c2: Projective) -> Projective
{
    let s = c1 * sk;
    let m = c2 - s;
    m
}