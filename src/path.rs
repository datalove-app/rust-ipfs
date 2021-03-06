use crate::error::{Error, TryError};
use crate::ipld::Ipld;
use cid::Cid;
use core::convert::{TryFrom, TryInto};
use libp2p::PeerId;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct IpfsPath {
    root: PathRoot,
    path: Vec<String>,
}

impl FromStr for IpfsPath {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self, Error> {
        let mut subpath = string.split('/');
        let empty = subpath.next().expect("there's always the first split");

        let root = if !empty.is_empty() {
            // by default if there is no prefix it's an ipfs or ipld path
            PathRoot::Ipld(Cid::try_from(empty)?)
        } else {
            let root_type = subpath.next();
            let key = subpath.next();

            match (empty, root_type, key) {
                ("", Some("ipfs"), Some(key)) => PathRoot::Ipld(Cid::try_from(key)?),
                ("", Some("ipld"), Some(key)) => PathRoot::Ipld(Cid::try_from(key)?),
                ("", Some("ipns"), Some(key)) => match PeerId::from_str(key).ok() {
                    Some(peer_id) => PathRoot::Ipns(peer_id),
                    None => PathRoot::Dns(key.to_string()),
                },
                _ => {
                    //todo!("empty: {:?}, root: {:?}, key: {:?}", empty, root_type, key);
                    return Err(IpfsPathError::InvalidPath(string.to_owned()).into());
                }
            }
        };

        let mut path = IpfsPath::new(root);
        path.push_split(subpath)
            .map_err(|_| IpfsPathError::InvalidPath(string.to_owned()))?;
        Ok(path)
    }
}

impl IpfsPath {
    pub fn new(root: PathRoot) -> Self {
        IpfsPath {
            root,
            path: Vec::new(),
        }
    }

    pub fn root(&self) -> &PathRoot {
        &self.root
    }

    pub fn set_root(&mut self, root: PathRoot) {
        self.root = root;
    }

    pub fn push_str(&mut self, string: &str) -> Result<(), Error> {
        if string.is_empty() {
            return Ok(());
        }

        self.push_split(string.split('/'))
            .map_err(|_| IpfsPathError::InvalidPath(string.to_owned()).into())
    }

    fn push_split<'a>(&mut self, split: impl Iterator<Item = &'a str>) -> Result<(), ()> {
        let mut split = split.peekable();
        while let Some(sub_path) = split.next() {
            if sub_path == "" {
                return if split.peek().is_none() {
                    // trim trailing
                    Ok(())
                } else {
                    // no empty segments in the middle
                    Err(())
                };
            }
            self.path.push(sub_path.to_owned());
        }
        Ok(())
    }

    pub fn sub_path(&self, string: &str) -> Result<Self, Error> {
        let mut path = self.to_owned();
        path.push_str(string)?;
        Ok(path)
    }

    pub fn into_sub_path(mut self, string: &str) -> Result<Self, Error> {
        self.push_str(string)?;
        Ok(self)
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.path.iter()
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }
}

impl fmt::Display for IpfsPath {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.root)?;
        for sub_path in &self.path {
            write!(fmt, "/{}", sub_path)?;
        }
        Ok(())
    }
}

impl TryFrom<&str> for IpfsPath {
    type Error = Error;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        IpfsPath::from_str(string)
    }
}

impl<T: Into<PathRoot>> From<T> for IpfsPath {
    fn from(root: T) -> Self {
        IpfsPath::new(root.into())
    }
}

impl TryInto<Cid> for IpfsPath {
    type Error = Error;

    fn try_into(self) -> Result<Cid, Self::Error> {
        match self.root().cid() {
            Some(cid) => Ok(cid.to_owned()),
            None => Err(anyhow::anyhow!("expected cid")),
        }
    }
}

impl TryInto<PeerId> for IpfsPath {
    type Error = Error;

    fn try_into(self) -> Result<PeerId, Self::Error> {
        match self.root().peer_id() {
            Some(peer_id) => Ok(peer_id.to_owned()),
            None => Err(anyhow::anyhow!("expected peer id")),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum PathRoot {
    Ipld(Cid),
    Ipns(PeerId),
    Dns(String),
}

impl fmt::Debug for PathRoot {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PathRoot::*;

        match self {
            Ipld(cid) => write!(fmt, "{}", cid),
            Ipns(pid) => write!(fmt, "{}", pid),
            Dns(name) => write!(fmt, "{:?}", name),
        }
    }
}

impl PathRoot {
    pub fn is_ipld(&self) -> bool {
        matches!(self, PathRoot::Ipld(_))
    }

    pub fn is_ipns(&self) -> bool {
        matches!(self, PathRoot::Ipns(_))
    }

    pub fn cid(&self) -> Option<&Cid> {
        match self {
            PathRoot::Ipld(cid) => Some(cid),
            _ => None,
        }
    }

    pub fn peer_id(&self) -> Option<&PeerId> {
        match self {
            PathRoot::Ipns(peer_id) => Some(peer_id),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.into()
    }
}

impl Into<Vec<u8>> for &PathRoot {
    fn into(self) -> Vec<u8> {
        match self {
            PathRoot::Ipld(cid) => cid.to_bytes(),
            PathRoot::Ipns(peer_id) => peer_id.as_bytes().to_vec(),
            PathRoot::Dns(domain) => domain.as_bytes().to_vec(),
        }
    }
}

impl fmt::Display for PathRoot {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let (prefix, key) = match self {
            PathRoot::Ipld(cid) => ("/ipfs/", cid.to_string()),
            PathRoot::Ipns(peer_id) => ("/ipns/", peer_id.to_base58()),
            PathRoot::Dns(domain) => ("/ipns/", domain.to_owned()),
        };
        write!(fmt, "{}{}", prefix, key)
    }
}

impl From<Cid> for PathRoot {
    fn from(cid: Cid) -> Self {
        PathRoot::Ipld(cid)
    }
}

impl From<PeerId> for PathRoot {
    fn from(peer_id: PeerId) -> Self {
        PathRoot::Ipns(peer_id)
    }
}

impl TryInto<Cid> for PathRoot {
    type Error = TryError;

    fn try_into(self) -> Result<Cid, Self::Error> {
        match self {
            PathRoot::Ipld(cid) => Ok(cid),
            _ => Err(TryError),
        }
    }
}

impl TryInto<PeerId> for PathRoot {
    type Error = TryError;

    fn try_into(self) -> Result<PeerId, Self::Error> {
        match self {
            PathRoot::Ipns(peer_id) => Ok(peer_id),
            _ => Err(TryError),
        }
    }
}

#[derive(Debug, Error)]
pub enum IpfsPathError {
    #[error("Invalid path {0:?}")]
    InvalidPath(String),
    #[error("Can't resolve {path:?}")]
    ResolveError { ipld: Ipld, path: String },
    #[error("Expected ipld path but found ipns path.")]
    ExpectedIpldPath,
}

#[cfg(test)]
mod tests {
    use super::IpfsPath;
    use std::convert::TryFrom;
    /*use super::*;
    use bitswap::Block;

    #[test]
    fn test_from() {
        let res = Block::from("hello").path("key/3").unwrap();

        let cid = Cid::new_v1(Codec::Raw, b"hello");
        let mut path = IpfsPath::new(PathRoot::Ipld(cid));
        path.push("key");
        path.push(3);

        assert_eq!(path, res);
    }

    #[test]
    fn test_from_errors() {
        let block = Block::from("hello");
        assert!(block.path("").is_ok());
        assert!(block.path("/").is_err());
        assert!(block.path("/abc").is_err());
        assert!(block.path("abc/").is_err());
        assert!(block.path("abc//de").is_err());
    }

    #[test]
    fn test_from_str() {
        let string = "/ipld/QmRN6wdp1S2A5EtjW9A3M1vKSBuQQGcgvuhoMUoEz4iiT5/key/3";
        let res = IpfsPath::from_str(string).unwrap();

        let cid = Block::from("hello").cid().to_owned();
        let mut path = IpfsPath::new(PathRoot::Ipld(cid));
        path.push("key");
        path.push(3);

        assert_eq!(path, res);
    }

    #[test]
    fn test_from_str_errors() {
        assert!(IpfsPath::from_str("").is_err());
        assert!(IpfsPath::from_str("/").is_err());
        assert!(IpfsPath::from_str("/QmRN").is_err());
    }

    #[test]
    fn test_to_string() {
        let path = Block::from("hello").path("key/3").unwrap();
        let res = "/ipfs/QmRN6wdp1S2A5EtjW9A3M1vKSBuQQGcgvuhoMUoEz4iiT5/key/3";
        assert_eq!(path.to_string(), res);
    }*/

    #[test]
    fn good_paths() {
        let good = [
            ("/ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n", 0),
            ("/ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a", 1),
            (
                "/ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a/b/c/d/e/f",
                6,
            ),
            (
                "QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a/b/c/d/e/f",
                6,
            ),
            ("QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n", 0),
            ("/ipld/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n", 0),
            ("/ipld/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a", 1),
            (
                "/ipld/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a/b/c/d/e/f",
                6,
            ),
            ("/ipns/QmSrPmbaUKA3ZodhzPWZnpFgcPMFWF4QsxXbkWfEptTBJd", 0),
            (
                "/ipns/QmSrPmbaUKA3ZodhzPWZnpFgcPMFWF4QsxXbkWfEptTBJd/a/b/c/d/e/f",
                6,
            ),
        ];

        for &(good, len) in &good {
            let p = IpfsPath::try_from(good).unwrap();
            assert_eq!(p.path().len(), len);
        }
    }

    #[test]
    fn bad_paths() {
        let bad = [
            "/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n",
            "/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/a",
            "/ipfs/foo",
            "/ipfs/",
            "ipfs/",
            "ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n",
            "/ipld/foo",
            "/ipld/",
            "ipld/",
            "ipld/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n",
        ];

        for &bad in &bad {
            IpfsPath::try_from(bad).unwrap_err();
        }
    }

    #[test]
    fn trailing_slash_is_ignored() {
        let paths = [
            "/ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/",
            "QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n/",
        ];
        for &path in &paths {
            let p = IpfsPath::try_from(path).unwrap();
            assert_eq!(p.path().len(), 0, "{:?} from {:?}", p, path);
        }
    }

    #[test]
    fn multiple_slashes_are_not_deduplicated() {
        // this used to be the behaviour in ipfs-http
        IpfsPath::try_from("/ipfs/QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n///a").unwrap_err();
    }
}
