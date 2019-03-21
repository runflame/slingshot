use super::VerificationKey;
use crate::errors::VMError;
use crate::transcript::TranscriptProtocol;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::CompressedRistretto;
use curve25519_dalek::scalar::Scalar;
use merlin::Transcript;

#[derive(Debug, Clone)]
pub struct Signature {
    pub s: Scalar,
    pub R: CompressedRistretto,
}

impl Signature {
    pub fn verify(&self, transcript: &Transcript, X: VerificationKey) -> Result<(), VMError> {
        let G = RISTRETTO_BASEPOINT_POINT;
        let mut transcript = transcript.clone();

        // Make c = H(X, R, m)
        // The message `m` has already been fed into the transcript
        let c = {
            transcript.commit_point(b"X", &X.0);
            transcript.commit_point(b"R", &self.R);
            transcript.challenge_scalar(b"c")
        };

        let X = X.0.decompress().ok_or(VMError::InvalidPoint)?;
        let R = self.R.decompress().ok_or(VMError::InvalidPoint)?;

        // Check sG = R + c * X
        if self.s * G == R + c * X {
            Ok(())
        } else {
            Err(VMError::PointOperationsFailed)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::errors::VMError;
    use crate::signature::signer::*;
    use crate::signature::{multikey::Multikey, VerificationKey};
    use curve25519_dalek::ristretto::CompressedRistretto;

    #[test]
    fn make_aggregated_pubkey() {
        // super secret, sshhh!
        let priv_keys = vec![
            Scalar::from(1u64),
            Scalar::from(2u64),
            Scalar::from(3u64),
            Scalar::from(4u64),
        ];
        let multikey = multikey_helper(&priv_keys).unwrap();

        let expected_pubkey = CompressedRistretto::from_slice(&[
            56, 92, 251, 79, 34, 221, 181, 222, 11, 112, 55, 45, 154, 242, 40, 250, 247, 1, 109,
            126, 150, 210, 181, 6, 117, 95, 44, 102, 38, 28, 144, 49,
        ]);

        assert_eq!(expected_pubkey, multikey.aggregated_key().0);
    }

    fn multikey_helper(priv_keys: &Vec<Scalar>) -> Option<Multikey> {
        let G = RISTRETTO_BASEPOINT_POINT;
        Multikey::new(
            priv_keys
                .iter()
                .map(|priv_key| VerificationKey((G * priv_key).compress()))
                .collect(),
        )
    }

    #[test]
    fn sign_message() {
        // super secret, sshhh!
        let priv_keys = vec![
            Scalar::from(1u64),
            Scalar::from(2u64),
            Scalar::from(3u64),
            Scalar::from(4u64),
        ];
        let multikey = multikey_helper(&priv_keys).unwrap();
        let m = b"message to sign".to_vec();

        sign_helper(priv_keys, multikey, m).unwrap();
    }

    fn sign_helper(
        privkeys: Vec<Scalar>,
        multikey: Multikey,
        m: Vec<u8>,
    ) -> Result<Signature, VMError> {
        let mut transcript = Transcript::new(b"signing test");
        transcript.commit_bytes(b"message", &m);
        let pubkeys: Vec<_> = privkeys
            .iter()
            .map(|privkey| VerificationKey((privkey * RISTRETTO_BASEPOINT_POINT).compress()))
            .collect();

        let (parties, precomms): (Vec<_>, Vec<_>) = privkeys
            .clone()
            .into_iter()
            .map(|x_i| Party::new(&transcript.clone(), x_i, multikey.clone(), pubkeys.clone()))
            .unzip();

        let (parties, comms): (Vec<_>, Vec<_>) = parties
            .into_iter()
            .map(|p| p.receive_precommitments(precomms.clone()))
            .unzip();

        let (parties, shares): (Vec<_>, Vec<_>) = parties
            .into_iter()
            .map(|p| p.receive_commitments(comms.clone()).unwrap())
            .unzip();

        let signatures: Vec<_> = parties
            .into_iter()
            .map(|p: PartyAwaitingShares| p.receive_shares(shares.clone()).unwrap())
            .collect();

        // Check that signatures from all parties are the same
        let cmp = &signatures[0];
        for sig in &signatures {
            assert_eq!(cmp.s, sig.s);
            assert_eq!(cmp.R, sig.R)
        }

        Ok(signatures[0].clone())
    }

    #[test]
    fn verify_sig() {
        // super secret, sshhh!
        let priv_keys = vec![
            Scalar::from(1u64),
            Scalar::from(2u64),
            Scalar::from(3u64),
            Scalar::from(4u64),
        ];
        let multikey = multikey_helper(&priv_keys).unwrap();
        let m = b"message to sign".to_vec();

        let signature = sign_helper(priv_keys, multikey.clone(), m.clone()).unwrap();

        let mut transcript = Transcript::new(b"signing test");
        transcript.commit_bytes(b"message", &m);

        assert!(signature
            .verify(&mut transcript, multikey.aggregated_key())
            .is_ok());
    }
}
