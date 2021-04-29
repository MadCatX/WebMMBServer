use serde::Deserialize;
use serde_json;

#[derive(Deserialize)]
enum BondMobility {
    Rigid,
    Torsion,
    Free,
}

#[derive(Deserialize)]
struct Mobilizer {
    pub bondMobility: BondMobility,
    pub chain: Option<String>,
    pub firstResidue: Option<i32>,
    pub lastResidue: Option<i32>,
}


type MobilizersVec = Vec<Mobilizer>;

impl std::convert::From<&BondMobility> for String {
    fn from(bm: &BondMobility) -> String {
        match bm {
            BondMobility::Rigid => String::from("Rigid"),
            BondMobility::Torsion => String::from("Torsion"),
            BondMobility::Free => String::from("Free"),
        }
    }
}

pub fn to_string_list(value: serde_json::Value) -> Result<Vec<String>, serde_json::Error> {
    let mut mobilizers: Vec<String> = Vec::new();

    match serde_json::from_value::<MobilizersVec>(value) {
        Ok(mobs) => {
            for m in mobs.iter() {
                let mut line = String::from(&m.bondMobility);

                if m.chain.is_some() {
                    line.push_str(format!(" {}", m.chain.as_ref().unwrap()).as_str());

                    if m.firstResidue.is_some() && m.lastResidue.is_some() {
                        line.push_str(format!(" {} {}", m.firstResidue.unwrap(), m.lastResidue.unwrap()).as_str());
                    }
                }

                mobilizers.push(line);
            }

            Ok(mobilizers)
        },
        Err(e) => Err(e),
    }
}
