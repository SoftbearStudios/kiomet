// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::death_reason::DeathReason;
use common::tower::TowerType;
use common::unit::Unit;
use core_protocol::id::LanguageId;
use core_protocol::id::LanguageId::*;
use core_protocol::name::PlayerAlias;
use std::borrow::Cow;
use yew_frontend::s;

pub trait TowerTranslation: Copy + Sized {
    s!(tower_label);

    fn tower_type_label(self, tower_type: TowerType) -> &'static str {
        use TowerType::*;
        match tower_type {
            Airfield => self.airfield_label(),
            Armory => self.armory_label(),
            Artillery => self.artillery_label(),
            Barracks => self.barracks_label(),
            Bunker => self.bunker_label(),
            //Capitol => "Capitol", // TODO
            Centrifuge => self.centrifuge_label(),
            City => self.city_label(),
            Cliff => self.cliff_label(),
            Ews => self.ews_label(),
            Factory => self.factory_label(),
            Generator => self.generator_label(),
            Headquarters => self.headquarters_label(),
            Helipad => self.helipad_label(),
            //Icbm => "ICBM",   // TODO
            //Laser => "Laser", // TODO
            Launcher => self.launcher_label(),
            //Metropolis => "Metropolis", // TODO
            Mine => self.mine_label(),
            Projector => self.projector_label(),
            Quarry => self.quarry_label(),
            Radar => self.radar_label(),
            Rampart => self.rampart_label(),
            Reactor => self.reactor_label(),
            Refinery => self.refinery_label(),
            Rocket => self.rocket_label(),
            Runway => self.runway_label(),
            Satellite => self.satellite_label(),
            Silo => self.silo_label(),
            Town => self.town_label(),
            Village => self.village_label(),
        }
    }

    // Towers
    s!(airfield_label);
    s!(armory_label);
    s!(artillery_label);
    s!(barracks_label);
    s!(bunker_label);
    s!(centrifuge_label);
    s!(city_label);
    s!(cliff_label);
    s!(ews_label);
    s!(factory_label);
    s!(generator_label);
    s!(headquarters_label);
    s!(helipad_label);
    s!(launcher_label);
    s!(mine_label);
    s!(projector_label);
    s!(quarry_label);
    s!(radar_label);
    s!(rampart_label);
    s!(reactor_label);
    s!(refinery_label);
    s!(rocket_label);
    s!(runway_label);
    s!(satellite_label);
    s!(silo_label);
    s!(town_label);
    s!(village_label);

    fn unit_label(self, unit: Unit) -> &'static str {
        use Unit::*;
        match unit {
            Bomber => self.bomber_label(),
            Chopper => self.chopper_label(),
            Emp => self.emp_label(),
            Fighter => self.fighter_label(),
            Nuke => self.nuke_label(),
            Ruler => self.ruler_label(),
            Shell => self.shell_label(),
            Shield => self.shield_label(),
            Soldier => self.soldier_label(),
            Tank => self.tank_label(),
        }
    }

    // Units
    s!(bomber_label);
    s!(chopper_label);
    s!(emp_label);
    s!(fighter_label);
    s!(nuke_label);
    s!(ruler_label);
    s!(shell_label);
    s!(shield_label);
    s!(soldier_label);
    s!(tank_label);

    fn death_reason(self, death_reason: DeathReason) -> String {
        match death_reason {
            DeathReason::RulerKilled { alias, unit } => self.ruler_killed(
                alias,
                // TODO don't use to_lowercase as it adds 32.6 kb to the binary.
                self.unit_label(unit),
            ),
        }
    }

    fn ruler_killed(self, alias: Option<PlayerAlias>, lower_unit_label: &str) -> String;

    // Tower menu actions.
    s!(demolish_hint);
    s!(request_alliance_hint);
    s!(cancel_alliance_hint);
    s!(break_alliance_hint);

    // Alerts
    s!(alert_capture_instruction);
    s!(alert_capture_hint);
    s!(alert_upgrade_instruction);
    s!(alert_upgrade_hint);
    fn alert_ruler_unsafe_instruction(self) -> String;
    s!(alert_ruler_unsafe_hint);
    fn alert_ruler_under_attack_warning(self) -> String;
    s!(alert_ruler_under_attack_hint);
    s!(alert_zombies_warning);
    s!(alert_zombies_hint);
    s!(alert_full_warning);
    s!(alert_full_hint);
    s!(alert_overflowing_warning);
    s!(alert_overflowing_hint);
}

impl TowerTranslation for LanguageId {
    fn tower_label(self) -> &'static str {
        match self {
            English => "Tower",
            Spanish => "Torre",
            French => "La tour",
            German => "Turm",
            Italian => "Torre",
            Russian => "Башня",
            Arabic => "برج",
            Hindi => "मीनार",
            SimplifiedChinese => "塔",
            Japanese => "タワー",
            Vietnamese => "Tòa tháp",
            Bork => "Bork",
        }
    }

    fn airfield_label(self) -> &'static str {
        match self {
            English => "Airfield",
            Spanish => "Aeródromo",
            French => "Aérodrome",
            German => "Flugplatz",
            Italian => "Aerodromo",
            Japanese => "飛行場",
            Russian => "Аэродром",
            Arabic => "مطار",
            Hindi => "एयरफील्ड",
            SimplifiedChinese => "机场",
            Vietnamese => "Sân bay",
            Bork => "Airbork",
        }
    }

    fn armory_label(self) -> &'static str {
        match self {
            English => "Armory",
            Spanish => "Arsenal",
            French => "Arsenal",
            German => "Rüstungsbetrieb",
            Italian => "Armeria",
            Japanese => "造兵廠",
            Russian => "Арсенал",
            Arabic => "المستودع",
            Hindi => "शस्रशाला",
            SimplifiedChinese => "坦克工厂",
            Vietnamese => "Kho vũ khí",
            Bork => "Armorbork",
        }
    }

    fn artillery_label(self) -> &'static str {
        match self {
            English => "Artillery",
            Spanish => "Artillería",
            French => "Artillerie",
            German => "Artillerie",
            Italian => "artiglieria",
            Japanese => "砲兵",
            Russian => "артиллерия",
            Arabic => "سلاح المدفعية",
            Hindi => "तोपें",
            SimplifiedChinese => "炮兵",
            Vietnamese => "pháo binh",
            Bork => "Borktillery",
        }
    }

    fn barracks_label(self) -> &'static str {
        match self {
            English => "Barracks",
            Spanish => "Cuartel",
            French => "Caserne",
            German => "Kaserne",
            Italian => "Caserma",
            Japanese => "兵営",
            Russian => "Казарма",
            Arabic => "الثكنات",
            Hindi => "बैरकों",
            SimplifiedChinese => "兵营",
            Vietnamese => "Doanh trại",
            Bork => "Borracks",
        }
    }

    fn bunker_label(self) -> &'static str {
        match self {
            English => "Bunker",
            Spanish => "Búnker",
            French => "Bunker",
            German => "Bunker",
            Italian => "Bunker",
            Japanese => "バンカー",
            Russian => "Бункер",
            Arabic => "القبو",
            Hindi => "बंकर",
            SimplifiedChinese => "掩体",
            Vietnamese => "Pháo đài",
            Bork => "Borker",
        }
    }

    fn centrifuge_label(self) -> &'static str {
        match self {
            English => "Centrifuge",
            Spanish => "Centrífuga",
            French => "Centrifugeuse",
            German => "Zentrifuge",
            Italian => "Centrifuga",
            Japanese => "遠心機",
            Russian => "Центрифуга",
            Arabic => "الطرد المركزي",
            Hindi => "अपकेंद्रित्र",
            SimplifiedChinese => "离心机",
            Vietnamese => "Máy ly tâm",
            Bork => "Borktrifuge",
        }
    }

    fn city_label(self) -> &'static str {
        match self {
            English => "City",
            Spanish => "Ciudad",
            French => "Cité",
            German => "Stadt",
            Italian => "Città",
            Japanese => "都市",
            Russian => "Город",
            Arabic => "مدينة",
            Hindi => "शहर",
            SimplifiedChinese => "城市",
            Vietnamese => "Thành phố",
            Bork => "Borkty",
        }
    }

    fn cliff_label(self) -> &'static str {
        match self {
            English => "Cliff",
            Spanish => "Acantilado",
            French => "Falaise",
            German => "Klippe",
            Italian => "Scogliera",
            Japanese => "崖",
            Russian => "Утес",
            Arabic => "جرف",
            Hindi => "टीला",
            SimplifiedChinese => "悬崖",
            Vietnamese => "Vách đá",
            Bork => "Borff",
        }
    }

    fn generator_label(self) -> &'static str {
        match self {
            English => "Generator",
            Spanish => "Generador",
            French => "Générateur",
            German => "Generator",
            Italian => "Generatore",
            Japanese => "ジェネレータ",
            Russian => "Генератор",
            Arabic => "مولد",
            Hindi => "जनक",
            SimplifiedChinese => "发电站",
            Vietnamese => "Generator",
            Bork => "Borkerator",
        }
    }

    fn headquarters_label(self) -> &'static str {
        match self {
            English => "Headquarters",
            Spanish => "Sede",
            French => "Quartier général",
            German => "Hauptquartier",
            Italian => "Sede",
            Japanese => "本部",
            Russian => "Штаб",
            Arabic => "المقر",
            Hindi => "मुख्यालय",
            SimplifiedChinese => "总部",
            Vietnamese => "Trụ sở",
            Bork => "Command Bork",
        }
    }

    fn helipad_label(self) -> &'static str {
        match self {
            English => "Helipad",
            Spanish => "Helipuerto",
            French => "Héliport",
            German => "Hubschrauberplatz",
            Italian => "Eliporto",
            Japanese => "ヘリポート",
            Russian => "вертолетная площадка",
            Arabic => "مهبط للطائرات العمودية",
            Hindi => "हैलीपैड",
            SimplifiedChinese => "直升机停机坪",
            Vietnamese => "Sân bay trực thăng",
            Bork => "Helibork",
        }
    }

    fn ews_label(self) -> &'static str {
        match self {
            English => "EWS",
            Spanish => "Sistema de Alerta Temprana",
            French => "Système d'alerte précoce",
            German => "Frühwarnsystem",
            Italian => "Sistema di allarme rapido",
            Japanese => "早期警戒システム",
            Russian => "Система раннего предупреждения",
            Arabic => "نظام الإنذار المبكر",
            Hindi => "पूर्व चेतावनी प्रणाली",
            SimplifiedChinese => "预警系统",
            Vietnamese => "Hệ thống cảnh báo sớm",
            Bork => "EBS",
        }
    }

    fn factory_label(self) -> &'static str {
        match self {
            English => "Factory",
            Spanish => "Fábrica",
            French => "Usine",
            German => "Fabrik",
            Italian => "Fabbrica",
            Russian => "Фабрика",
            Arabic => "مصنع",
            Hindi => "कारखाना",
            SimplifiedChinese => "工厂",
            Japanese => "工場",
            Vietnamese => "Nhà máy",
            Bork => "Borktory",
        }
    }

    fn launcher_label(self) -> &'static str {
        match self {
            English => "Launcher",
            Spanish => "Lanzacohetes",
            French => "Lance-roquettes",
            German => "Raketenwerfer",
            Italian => "Lanciarazzi",
            Russian => "гранатомет",
            Arabic => "راجمة",
            Hindi => "राकेट प्रक्षेपक",
            SimplifiedChinese => "火箭发射器",
            Japanese => "ロケット発射筒",
            Vietnamese => "Phóng tên lửa",
            Bork => "Launcherk",
        }
    }

    fn mine_label(self) -> &'static str {
        match self {
            English => "Mine",
            Spanish => "Mina",
            French => "Mine",
            German => "Mine",
            Italian => "Miniera",
            Russian => "Шахта",
            Arabic => "الخاص بي",
            Hindi => "माइनशाफ्ट",
            SimplifiedChinese => "矿山",
            Japanese => "機雷",
            Vietnamese => "Mỏ",
            Bork => "Bork Mine",
        }
    }

    fn projector_label(self) -> &'static str {
        match self {
            English => "Projector",
            Spanish => "Proyector",
            French => "Projecteur",
            German => "Beamer",
            Italian => "Proiettore",
            Russian => "проектор",
            Arabic => "كشاف ضوئي",
            Hindi => "प्रक्षेपक",
            SimplifiedChinese => "投影仪",
            Japanese => "プロジェクター",
            Vietnamese => "Máy chiếu",
            Bork => "Porjector",
        }
    }

    fn rampart_label(self) -> &'static str {
        match self {
            English => "Rampart",
            Spanish => "Muralla",
            French => "Rempart",
            German => "Bastion",
            Italian => "Bastione",
            Russian => "бастион",
            Arabic => "سور",
            Hindi => "किले की दीवार",
            SimplifiedChinese => "壁垒",
            Japanese => "堅塁",
            Vietnamese => "bờ lủy",
            Bork => "Rambork",
        }
    }

    fn reactor_label(self) -> &'static str {
        match self {
            English => "Reactor",
            Spanish => "Reactor",
            French => "Réacteur",
            German => "Reaktor",
            Italian => "Nucleare",
            Russian => "Аэс",
            Arabic => "المفاعل",
            Hindi => "रिएक्टर",
            SimplifiedChinese => "反应堆",
            Japanese => "原子炉",
            Vietnamese => "Lò phản ứng",
            Bork => "Reactbork",
        }
    }

    fn refinery_label(self) -> &'static str {
        match self {
            English => "Refinery",
            Spanish => "Refinería",
            French => "Raffinerie",
            German => "Raffinerie",
            Italian => "Raffineria",
            Japanese => "製油所",
            Russian => "Химический завод",
            Arabic => "مصفاة",
            Hindi => "रिफाइनरी",
            SimplifiedChinese => "炼油厂",
            Vietnamese => "Nhà máy lọc dầu",
            Bork => "Reborkery",
        }
    }

    fn radar_label(self) -> &'static str {
        match self {
            English => "Radar",
            Spanish => "Radar",
            French => "Radar",
            German => "Radar",
            Italian => "Radar",
            Russian => "Радар",
            Arabic => "رادار",
            Hindi => "राडार",
            SimplifiedChinese => "雷达",
            Japanese => "レーダー",
            Vietnamese => "Radar",
            Bork => "Radar Bork",
        }
    }

    fn ruler_label(self) -> &'static str {
        match self {
            English => "King",
            Spanish => "Gobernante",
            French => "Souverain",
            German => "Herrscher",
            Italian => "Capo",
            Japanese => "天皇",
            Russian => "Царь",
            Arabic => "ملِك",
            Hindi => "राजा",
            SimplifiedChinese => "皇帝",
            Vietnamese => "Chủ tịch",
            Bork => "BORK",
        }
    }

    fn rocket_label(self) -> &'static str {
        match self {
            English => "Rocket",
            Spanish => "Cohete",
            French => "Fusée",
            German => "Rakete",
            Italian => "Razzo",
            Japanese => "ロケット",
            Russian => "Ракета",
            Arabic => "صاروخ",
            Hindi => "राकेट",
            SimplifiedChinese => "火箭",
            Vietnamese => "Tên lửa",
            Bork => "Borket",
        }
    }

    fn runway_label(self) -> &'static str {
        // May translate as "Airstrip" instead.
        match self {
            English => "Runway",
            Spanish => "Pista",
            French => "Piste",
            German => "Landebahn",
            Italian => "Pista",
            Japanese => "滑走路",
            Russian => "взлетная полоса",
            Arabic => "المدرج",
            Hindi => "हवाई पट्टी",
            SimplifiedChinese => "跑道",
            Vietnamese => "Đường băng",
            Bork => "Borkway",
        }
    }

    fn quarry_label(self) -> &'static str {
        match self {
            English => "Quarry",
            Spanish => "Cantera",
            French => "Carrière",
            German => "Steinbruch",
            Italian => "Cava",
            Russian => "Карьер",
            Arabic => "مقلع",
            Hindi => "शिकार",
            SimplifiedChinese => "采石场",
            Japanese => "切り出す",
            Vietnamese => "Mỏ đá",
            Bork => "Borrky",
        }
    }

    fn satellite_label(self) -> &'static str {
        match self {
            English => "Satellite",
            Spanish => "Satélite",
            French => "Satellite",
            German => "Satellit",
            Italian => "Satellitare",
            Russian => "Спутник",
            Arabic => "الأقمار الصناعية",
            Hindi => "उपग्रह",
            SimplifiedChinese => "卫星",
            Japanese => "衛生",
            Vietnamese => "Vệ tinh",
            Bork => "Borkellite",
        }
    }

    fn silo_label(self) -> &'static str {
        match self {
            English => "Silo",
            Spanish => "Silo de misiles",
            French => "Silo à missiles",
            German => "Raketensilo",
            Italian => "Silo missilistico",
            Russian => "Ракетная шахта",
            Arabic => "صومعة الصواريخ",
            Hindi => "साइलो",
            SimplifiedChinese => "导弹发射井",
            Japanese => "ミサイルサイロ",
            Vietnamese => "Bệ phóng tên lửa",
            Bork => "Bork Silo",
        }
    }

    fn town_label(self) -> &'static str {
        match self {
            English => "Town",
            Spanish => "Municipio",
            French => "Commune",
            German => "Stadt",
            Italian => "Paese",
            Russian => "Город",
            Arabic => "مدينة",
            Hindi => "कस्बा",
            SimplifiedChinese => "城镇",
            Japanese => "郷",
            Vietnamese => "Xã",
            Bork => "Borktown",
        }
    }

    fn village_label(self) -> &'static str {
        match self {
            English => "Village",
            Spanish => "Pueblo",
            French => "Village",
            German => "Dorf",
            Italian => "Villaggio",
            Russian => "Деревня",
            Arabic => "قرية",
            Hindi => "गांव",
            SimplifiedChinese => "村庄",
            Japanese => "村",
            Vietnamese => "Làng",
            Bork => "Borkville",
        }
    }

    fn emp_label(self) -> &'static str {
        match self {
            English => "EMP",
            Spanish => "EMP",
            French => "PEM",
            German => "EMP",
            Italian => "EMP",
            Russian => "Электромагнитный импульс",
            Arabic => "كهرومغناطيسية",
            Hindi => "विद्युत चुम्बकीय नाड़ी",
            SimplifiedChinese => "电磁脉冲",
            Japanese => "電磁パルス",
            Vietnamese => "Xung điện từ",
            Bork => "EMB",
        }
    }

    fn shell_label(self) -> &'static str {
        match self {
            English => "Shell",
            Spanish => "Proyectil",
            French => "Obus",
            German => "Granate",
            Italian => "Proiettile",
            Russian => "снаряд",
            Arabic => "قذيفة",
            Hindi => "खोल",
            SimplifiedChinese => "弹",
            Japanese => "弾",
            Vietnamese => "đạn trái phá",
            Bork => "Borkk",
        }
    }

    fn shield_label(self) -> &'static str {
        match self {
            English => "Shield",
            Spanish => "Escudo",
            French => "Bouclier",
            German => "Schild",
            Italian => "Scudo",
            Russian => "Щит",
            Arabic => "درع",
            Hindi => "कवच",
            SimplifiedChinese => "防护盾",
            Japanese => "シールド",
            Vietnamese => "Khiên",
            Bork => "Bork Field",
        }
    }

    fn soldier_label(self) -> &'static str {
        match self {
            English => "Soldier",
            Spanish => "Soldado",
            French => "Soldat",
            German => "Soldat",
            Italian => "Soldato",
            Russian => "Солдат",
            Arabic => "جندي",
            Hindi => "सैनिक",
            SimplifiedChinese => "士兵",
            Japanese => "兵士",
            Vietnamese => "Quân nhân",
            Bork => "Borker",
        }
    }

    fn tank_label(self) -> &'static str {
        match self {
            English => "Tank",
            Spanish => "Tanque",
            French => "Tank",
            German => "Panzer",
            Italian => "Carro armato",
            Russian => "Танк",
            Arabic => "دبابة",
            Hindi => "टैंक",
            SimplifiedChinese => "坦克",
            Japanese => "戦車",
            Vietnamese => "Xe tăng",
            Bork => "Bork",
        }
    }

    fn fighter_label(self) -> &'static str {
        match self {
            English => "Fighter",
            Spanish => "Avión de combate",
            French => "Avion de chasse",
            German => "Kampfflugzeug",
            Italian => "Aereo da combattimento",
            Russian => "Истребитель",
            Arabic => "طائرة مقاتلة",
            Hindi => "लड़ाकू",
            SimplifiedChinese => "",
            Japanese => "战斗机",
            Vietnamese => "Máy bay chiến đấu",
            Bork => "Smol Borker",
        }
    }

    fn bomber_label(self) -> &'static str {
        match self {
            English => "Bomber",
            Spanish => "Bombardero",
            French => "Bombardier",
            German => "Bomber",
            Italian => "Bombardiere",
            Russian => "Бомбардировщик",
            Arabic => "مهاجم",
            Hindi => "बमवर्षक",
            SimplifiedChinese => "轰炸机",
            Japanese => "爆撃機",
            Vietnamese => "Máy bay ném bom",
            Bork => "Borker",
        }
    }

    fn chopper_label(self) -> &'static str {
        match self {
            English => "Chopper",
            Spanish => "Helicóptero",
            French => "Hélicoptère",
            German => "Hubschrauber",
            Italian => "Elicottero",
            Japanese => "ヘリコプター",
            Russian => "Вертолет",
            Arabic => "هليكوبتر",
            Hindi => "हेलीकॉप्टर",
            SimplifiedChinese => "直升机",
            Vietnamese => "Trực thăng",
            Bork => "Choppy Bork",
        }
    }

    fn nuke_label(self) -> &'static str {
        match self {
            English => "Nuke",
            Spanish => "Nuke",
            French => "Micro-onde",
            German => "Atomwaffe",
            Italian => "Nuke",
            Russian => "Ядерная бомба",
            Arabic => "النوويه",
            Hindi => "परमाणु",
            SimplifiedChinese => "核武器",
            Japanese => "核兵器",
            Vietnamese => "Nuke",
            Bork => "Borke",
        }
    }

    fn demolish_hint(self) -> &'static str {
        match self {
            English => "Demolish",
            Spanish => "Demolerlo",
            French => "Démolissez-le",
            German => "Zerstören",
            Italian => "Demolire",
            Russian => "Снести",
            Arabic => "هدم",
            Hindi => "ध्वस्त",
            SimplifiedChinese => "",
            Japanese => "取り壊す",
            Vietnamese => "Phá hủy",
            Bork => "",
        }
    }

    fn request_alliance_hint(self) -> &'static str {
        match self {
            English => "Request alliance",
            Spanish => "Solicitar alianza",
            French => "Demande d'alliance",
            German => "Allianz anfordern",
            Italian => "Richiedi alleanza",
            Japanese => "同盟を要請する",
            Russian => "Запросить альянс",
            Arabic => "طلب التحالف",
            Hindi => "गठबंधन का अनुरोध करें",
            SimplifiedChinese => "请求联盟",
            Vietnamese => "yêu cầu liên minh",
            Bork => "Bork?",
        }
    }

    fn cancel_alliance_hint(self) -> &'static str {
        match self {
            English => "Cancel request",
            Spanish => "Cancelar petición",
            French => "Demande d'annulation",
            German => "Anfrage abbrechen",
            Italian => "Richiesta cancellata",
            Japanese => "リクエストのキャンセル",
            Russian => "Отменить запрос",
            Arabic => "إلغاء الطلب",
            Hindi => "अनुरोध को रद्द करें",
            SimplifiedChinese => "取消请求",
            Vietnamese => "Hủy yêu cầu",
            Bork => "Krob",
        }
    }

    fn break_alliance_hint(self) -> &'static str {
        match self {
            English => "Break alliance",
            Spanish => "Romper alianza",
            French => "Rompre l'alliance",
            German => "Bündnis brechen",
            Italian => "Rompi l'alleanza",
            Japanese => "同盟を破る",
            Russian => "Разорвать союз",
            Arabic => "كسر التحالف",
            Hindi => "गठबंधन तोड़ो",
            SimplifiedChinese => "打破联盟",
            Vietnamese => "Phá vỡ liên minh",
            Bork => "Krob",
        }
    }

    fn alert_capture_instruction(self) -> &'static str {
        match self {
            English => "Capture more towers",
            Spanish => "Captura más torres",
            French => "Capturez plus de tours",
            German => "Erobere mehr Türme",
            Italian => "Cattura più torri",
            Russian => "Захватите больше башен",
            Arabic => "التقط المزيد من الأبراج",
            Hindi => "अधिक टावर कैप्चर करें",
            SimplifiedChinese => "占领更多的塔",
            Japanese => "より多くの塔を占領",
            Vietnamese => "Chụp nhiều tháp hơn",
            Bork => "Bork more borks",
        }
    }

    fn alert_capture_hint(self) -> &'static str {
        match self {
            English => "Drag units from your towers to outside your borders",
            Spanish => "Arrastra unidades desde tus torres hasta fuera de tus fronteras",
            French => "Faites glisser des unités de vos tours vers l'extérieur de vos frontières",
            German => "Ziehen Sie Einheiten von Ihren Türmen über Ihre Grenzen hinaus",
            Italian => "Trascina le unità dalle tue torri al di fuori dei tuoi confini",
            Russian => "Перетаскивайте отряды из своих башен за пределы своих границ.",
            Arabic => "اسحب الوحدات من أبراجك إلى خارج حدودك",
            Hindi => "अपने टावरों से इकाइयों को अपनी सीमाओं के बाहर खींचें",
            SimplifiedChinese => "将单位从你的塔拖到你的边界之外",
            Japanese => "ユニットをタワーから国境の外にドラッグします",
            Vietnamese => "Kéo các đơn vị từ tháp của bạn ra bên ngoài biên giới của bạn",
            Bork => "Drag borks from your borks to outside your borkders",
        }
    }

    fn alert_upgrade_instruction(self) -> &'static str {
        match self {
            English => "Upgrade a tower",
            Spanish => "Mejora una torre",
            French => "Améliorer une tour",
            German => "Verbessere einen Turm",
            Italian => "Migliora una torre",
            Russian => "Улучшить башню",
            Arabic => "قم بترقية البرج",
            Hindi => "एक टावर अपग्रेड करें",
            SimplifiedChinese => "升级塔",
            Japanese => "タワーをアップグレードする",
            Vietnamese => "Nâng cấp tháp",
            Bork => "Upgrade a bork",
        }
    }

    fn alert_upgrade_hint(self) -> &'static str {
        match self {
            English => "Click a tower to show upgrade options",
            Spanish => "Haga clic en una torre para mostrar las opciones de actualización",
            French => "Cliquez sur une tour pour afficher les options de mise à niveau",
            German => "Klicken Sie auf einen Turm, um Upgrade-Optionen anzuzeigen",
            Italian => "Fare clic su una torre per visualizzare le opzioni di aggiornamento",
            Russian => "Fare clic su una torre per visualizzare le opzioni di aggiornamento",
            Arabic => "انقر فوق برج لإظهار خيارات الترقية",
            Hindi => "अपग्रेड विकल्प दिखाने के लिए टावर पर क्लिक करें",
            SimplifiedChinese => "单击塔以显示升级选项",
            Japanese => "タワーをクリックしてアップグレード オプションを表示します",
            Vietnamese => "Nhấp vào tháp để hiển thị các tùy chọn nâng cấp",
            Bork => "Click a bork to show upgrade options",
        }
    }

    fn alert_ruler_unsafe_instruction(self) -> String {
        let ruler = self.ruler_label();
        match self {
            English | Bork => format!("Move your {ruler} to safety"),
            Spanish => format!("Mueve tu {ruler} a un lugar seguro"),
            French => format!("Déplacez votre {ruler} en lieu sûr"),
            German => format!("Bringe deinen {ruler} in Sicherheit"),
            Italian => format!("Sposta il tuo {ruler} al sicuro"),
            Russian => format!("Переместите свою {ruler} в безопасное место"),
            Arabic => format!("انقل {ruler} إلى مكان آمن"),
            Hindi => format!("अपने {ruler} को सुरक्षित स्थान पर ले जाएं"),
            SimplifiedChinese => format!("将您的 {ruler} 移至安全地带"),
            Japanese => format!("{ruler} を安全な場所に移動します"),
            Vietnamese => format!("Di chuyển {ruler} của bạn đến nơi an toàn"),
        }
    }

    fn alert_ruler_unsafe_hint(self) -> &'static str {
        // FIXME: Redundant tower names?
        match self {
            English => "Shielded Headquarters or Bunkers near the center of your territory provide the most protection",
            Spanish => "Los cuarteles generales o búnkeres blindados cerca del centro de su territorio brindan la mayor protección",
            French => "Les quartiers généraux blindés ou les bunkers situés près du centre de votre territoire offrent la meilleure protection",
            German => "Abgeschirmte Hauptquartiere oder Bunker in der Nähe des Zentrums Ihres Territoriums bieten den besten Schutz",
            Italian => "Quartieri centrali schermati o bunker vicino al centro del tuo territorio forniscono la massima protezione",
            Russian => "Экранированные штаб-квартиры или бункеры в центре вашей территории обеспечивают наибольшую защиту.",
            Arabic => "توفر المقرات المحمية أو المخابئ بالقرب من وسط منطقتك أكبر قدر من الحماية",
            Hindi => "आपके क्षेत्र के केंद्र के पास परिरक्षित मुख्यालय या बंकर सबसे अधिक सुरक्षा प्रदान करते हैं",
            SimplifiedChinese => "靠近您的领土中心的屏蔽总部或掩体提供最大的保护",
            Japanese => "あなたの領土の中心近くにあるシールドされた本部またはバンカーは、最も保護を提供します",
            Vietnamese => "Trụ sở được che chắn hoặc các boongke gần trung tâm lãnh thổ của bạn cung cấp khả năng bảo vệ tối đa",
            Bork => "Shielded Command Bork or Borkers near the center of your territory provide the most protection",
        }
    }

    fn alert_ruler_under_attack_warning(self) -> String {
        let ruler = self.ruler_label();
        match self {
            English => format!("Your {ruler} is under attack!"),
            Spanish => format!("¡Tu {ruler} está bajo ataque!"),
            French => format!("Votre {ruler} est attaqué!"),
            German => format!("Dein {ruler} wird angegriffen!"),
            Italian => format!("Il tuo {ruler} è sotto attacco!"),
            Russian => format!("Ваш {ruler} атакован!"),
            Arabic => format!("{ruler} الخاص بك يتعرض للهجوم!"),
            Hindi => format!("आपके {ruler} पर हमला हो रहा है!"),
            SimplifiedChinese => format!("你的 {ruler} 受到攻击！"),
            Japanese => format!("あなたの {ruler} が攻撃を受けています!"),
            Vietnamese => format!("{ruler} của bạn đang bị tấn công!"),
            Bork => format!("Your {ruler} is getting borked!"),
        }
    }

    fn alert_ruler_under_attack_hint(self) -> &'static str {
        match self {
            English => "If they die, you lose the game",
            Spanish => "Si muere, pierdes el juego.",
            French => "Si meurt, vous perdez la partie",
            German => "Wenn Er stribt, verlieren Sie das Spiel",
            Italian => "Se muore, perdi la partita",
            Russian => "Если умирает, вы проигрываете игру",
            Arabic => "إذا مات ، تخسر اللعبة",
            Hindi => "यदि वे मर जाते हैं, तो आप खेल हार जाते हैं",
            SimplifiedChinese => "如果死了，你输掉比赛",
            Japanese => "死亡した場合、ゲームに負けます",
            Vietnamese => "Nếu chết, bạn sẽ thua trò chơi",
            Bork => "If they are borked, the game is borked.",
        }
    }

    fn alert_zombies_warning(self) -> &'static str {
        match self {
            English => "Zombies sighted",
            Spanish => "Zombis avistados",
            French => "Zombies aperçus",
            German => "Zombies gesichtet",
            Italian => "Zombie avvistati",
            Russian => "Зомби замечены",
            Arabic => "الزومبي شوهد",
            Hindi => "लाश देखी गई",
            SimplifiedChinese => "看到僵尸",
            Japanese => "目撃されたゾンビ",
            Vietnamese => "Thây ma nhìn thấy",
            Bork => "Zomborks sighted",
        }
    }

    fn alert_zombies_hint(self) -> &'static str {
        match self {
            English => "Escape them by moving in the opposite direction",
            Spanish => "Escapa de ellos moviéndote en la dirección opuesta.",
            French => "Échappez-leur en vous déplaçant dans la direction opposée",
            German => "Entkomme ihnen, indem du dich in die entgegengesetzte Richtung bewegst",
            Italian => "Fuggili muovendoti nella direzione opposta",
            Russian => "Избегайте их, двигаясь в противоположном направлении",
            Arabic => "اهرب منهم بالتحرك في الاتجاه المعاكس",
            Hindi => "विपरीत दिशा में आगे बढ़ते हुए उनसे बचो",
            SimplifiedChinese => "通过向相反方向移动来逃脱它们",
            Japanese => "反対方向に移動して逃げる",
            Vietnamese => "Thoát khỏi chúng bằng cách di chuyển theo hướng ngược lại",
            Bork => "Escape them by borking in the opposite direction",
        }
    }

    fn alert_full_warning(self) -> &'static str {
        match self {
            English => "A tower is full",
            Spanish => "Una torre está llena",
            French => "Une tour est pleine",
            German => "Ein Turm ist voll",
            Italian => "Una torre è piena",
            Russian => "Башня полна",
            Arabic => "برج ممتلئ",
            Hindi => "एक टावर भरा हुआ है",
            SimplifiedChinese => "一座塔已满",
            Japanese => "タワーがいっぱいです",
            Vietnamese => "Một tòa tháp đã đầy",
            Bork => "A bork is borked",
        }
    }

    fn alert_full_hint(self) -> &'static str {
        match self {
            English => "Drag away units to make room for more",
            Spanish => "Arrastra unidades para hacer espacio para más",
            French => "Faites glisser les unités pour faire de la place pour plus",
            German => "Ziehen Sie Einheiten weg, um Platz für mehr zu schaffen",
            Italian => "Trascina le unità per fare spazio ad altre",
            Russian => "Перетащите единицы, чтобы освободить место для большего количества",
            Arabic => "اسحب الوحدات بعيدًا لإفساح المجال للمزيد",
            Hindi => "अधिक के लिए जगह बनाने के लिए इकाइयों को दूर खींचें",
            SimplifiedChinese => "拖走单位以腾出更多空间",
            Japanese => "ユニットを引き離してスペースを空けます",
            Vietnamese => "Kéo đơn vị ra xa để có thêm chỗ",
            Bork => "Drag borks away to make room for more borks",
        }
    }

    fn alert_overflowing_warning(self) -> &'static str {
        match self {
            English => "A tower is overflowing",
            Spanish => "Una torre se desborda",
            French => "Une tour déborde",
            German => "Ein Turm quillt über",
            Italian => "Una torre trabocca",
            Russian => "Башня переполнена",
            Arabic => "البرج يفيض",
            Hindi => "एक टावर ओवरफ्लो हो रहा है",
            SimplifiedChinese => "一座塔溢出来",
            Japanese => "タワーがあふれています",
            Vietnamese => "Một tòa tháp đang tràn",
            Bork => "A bork is borking",
        }
    }

    fn alert_overflowing_hint(self) -> &'static str {
        match self {
            English => "Drag away units to stop them from disappearing",
            Spanish => "Arrastra unidades para evitar que desaparezcan",
            French => "Faites glisser les unités pour les empêcher de disparaître",
            German => "Ziehen Sie Einheiten weg, um zu verhindern, dass sie verschwinden",
            Italian => "Trascina le unità per impedire che scompaiano",
            Russian => "Перетащите юнитов, чтобы они не исчезли",
            Arabic => "اسحب الوحدات بعيدًا لمنعها من الاختفاء",
            Hindi => "इकाइयों को गायब होने से रोकने के लिए उन्हें दूर खींचें",
            SimplifiedChinese => "拖走单位以阻止它们消失",
            Japanese => "ユニットをドラッグして、ユニットが消えるのを防ぎます",
            Vietnamese => "Kéo các đơn vị đi để ngăn chúng biến mất",
            Bork => "Drag borks away to stop them from borking",
        }
    }

    fn ruler_killed(self, alias: Option<PlayerAlias>, unit: &str) -> String {
        let ruler = self.ruler_label();
        let owner = alias.map_or(
            match self {
                English => "zombie",
                Spanish => "zombi",
                French => "zombi",
                German => "zombie",
                Italian => "zombie",
                Russian => "живой мертвец",
                Arabic => "الاموات الاحياء",
                Hindi => "ज़ोंबी",
                SimplifiedChinese => "僵尸",
                Japanese => "ゾンビ",
                Vietnamese => "thây ma",
                Bork => "zombie",
            }
            .into(),
            |alias| {
                Cow::Owned(match self {
                    English => format!("{alias}'s"),
                    Spanish => format!("de {alias}"),
                    French => format!("de {alias}"),
                    German => format!("{alias}s"),
                    Italian => format!("di {alias}"),
                    Russian => format!("{alias}"),
                    Arabic => format!("{alias}"),
                    Hindi => format!("{alias} का"),
                    SimplifiedChinese => format!("{alias}的"),
                    Japanese => format!("{alias}の"),
                    Vietnamese => format!("của {alias}"),
                    Bork => format!("{alias}'s"),
                })
            },
        );

        match self {
            English => format!("{ruler} killed by {owner} {unit}!"),
            Spanish => format!("¡{ruler} es asesinado por {unit} {owner}!"),
            French => format!("{ruler} tué par {unit} {owner}!"),
            German => format!("{ruler} von {owner} {unit} getötet!"),
            Italian => format!("{ruler} ucciso da {unit} {owner}!"),
            Russian => format!("{ruler} был убит {owner} {unit}!"),
            Arabic => format!("قتل {ruler} على يد {owner} {unit}"),
            Hindi => format!("{ruler} को {owner} के {unit} ने मार डाला था"),
            SimplifiedChinese => format!("{ruler}被{owner}{unit}杀死!"),
            Japanese => format!("{ruler}は{owner}{unit}によって殺されました!"),
            Vietnamese => format!("{ruler} bị giết bởi {unit} {owner}!"),
            Bork => format!("{ruler} borked by {owner} {unit}!"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::translation::TowerTranslation;
    use common::death_reason::DeathReason;
    use common::unit::Unit;
    use core_protocol::id::LanguageId;
    use core_protocol::name::PlayerAlias;

    #[test]
    fn test_death_reason() {
        let reason = DeathReason::RulerKilled {
            alias: Some(PlayerAlias::new_unsanitized("Bob")),
            unit: Unit::Soldier,
        };
        for id in LanguageId::iter() {
            println!("{}", id.death_reason(reason))
        }

        let reason = DeathReason::RulerKilled {
            alias: None,
            unit: Unit::Soldier,
        };
        for id in LanguageId::iter() {
            println!("{}", id.death_reason(reason))
        }
    }
}
