//! Rank names for every rank ladder in the `Rank` journal event.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RankKind {
    Combat,
    Trade,
    Explore,
    Soldier,
    Exobiologist,
    Empire,
    Federation,
    Cqc,
}

impl RankKind {
    pub const ALL: [RankKind; 8] = [
        RankKind::Combat,
        RankKind::Trade,
        RankKind::Explore,
        RankKind::Soldier,
        RankKind::Exobiologist,
        RankKind::Cqc,
        RankKind::Federation,
        RankKind::Empire,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RankKind::Combat => "Combat",
            RankKind::Trade => "Trade",
            RankKind::Explore => "Exploration",
            RankKind::Soldier => "Mercenary",
            RankKind::Exobiologist => "Exobiologist",
            RankKind::Empire => "Empire",
            RankKind::Federation => "Federation",
            RankKind::Cqc => "CQC",
        }
    }

    pub fn names(self) -> &'static [&'static str] {
        match self {
            RankKind::Combat => &COMBAT,
            RankKind::Trade => &TRADE,
            RankKind::Explore => &EXPLORE,
            RankKind::Soldier => &SOLDIER,
            RankKind::Exobiologist => &EXOBIOLOGIST,
            RankKind::Empire => &EMPIRE,
            RankKind::Federation => &FEDERATION,
            RankKind::Cqc => &CQC,
        }
    }

    pub fn name_of(self, rank: u32) -> &'static str {
        self.names().get(rank as usize).copied().unwrap_or("?")
    }

    pub fn max(self) -> u32 {
        self.names().len() as u32 - 1
    }
}

const COMBAT: [&str; 14] = [
    "Harmless", "Mostly Harmless", "Novice", "Competent", "Expert", "Master", "Dangerous", "Deadly", "Elite", "Elite I",
    "Elite II", "Elite III", "Elite IV", "Elite V",
];
const TRADE: [&str; 14] = [
    "Penniless", "Mostly Penniless", "Peddler", "Dealer", "Merchant", "Broker", "Entrepreneur", "Tycoon", "Elite",
    "Elite I", "Elite II", "Elite III", "Elite IV", "Elite V",
];
const EXPLORE: [&str; 14] = [
    "Aimless", "Mostly Aimless", "Scout", "Surveyor", "Trailblazer", "Pathfinder", "Ranger", "Pioneer", "Elite", "Elite I",
    "Elite II", "Elite III", "Elite IV", "Elite V",
];
const SOLDIER: [&str; 14] = [
    "Defenceless", "Mostly Defenceless", "Rookie", "Soldier", "Gunslinger", "Warrior", "Gladiator", "Deadeye", "Elite",
    "Elite I", "Elite II", "Elite III", "Elite IV", "Elite V",
];
const EXOBIOLOGIST: [&str; 14] = [
    "Directionless", "Mostly Directionless", "Compiler", "Collector", "Cataloguer", "Taxonomist", "Ecologist",
    "Geneticist", "Elite", "Elite I", "Elite II", "Elite III", "Elite IV", "Elite V",
];
const EMPIRE: [&str; 15] = [
    "None", "Outsider", "Serf", "Master", "Squire", "Knight", "Lord", "Baron", "Viscount", "Count", "Earl", "Marquis",
    "Duke", "Prince", "King",
];
const FEDERATION: [&str; 15] = [
    "None", "Recruit", "Cadet", "Midshipman", "Petty Officer", "Chief Petty Officer", "Warrant Officer", "Ensign",
    "Lieutenant", "Lt. Commander", "Post Commander", "Post Captain", "Rear Admiral", "Vice Admiral", "Admiral",
];
const CQC: [&str; 14] = [
    "Helpless", "Mostly Helpless", "Amateur", "Semi Professional", "Professional", "Champion", "Hero", "Legend",
    "Elite", "Elite I", "Elite II", "Elite III", "Elite IV", "Elite V",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(RankKind::Explore.name_of(6), "Ranger");
        assert_eq!(RankKind::Empire.name_of(7), "Baron");
        assert_eq!(RankKind::Combat.name_of(99), "?");
    }
}
