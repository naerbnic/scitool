use std::{io, vec};

use bitter::BitReader;

use crate::util::{
    block::{Block, BlockSource},
    data_reader::{DataReader, FromBlockSource},
};

use super::mapfile::ResourceLocation;

/// A resource entry header in a data file.
///
/// This is based on the SCI1.1 data file format.
#[derive(Debug)]
pub struct RawEntryHeader {
    res_type: u8,
    res_number: u16,
    packed_size: u16,
    unpacked_size: u16,
    compression_type: u16,
}

impl FromBlockSource for RawEntryHeader {
    fn read_size() -> usize {
        9
    }

    fn parse<R>(mut reader: R) -> io::Result<Self>
    where
        R: DataReader,
    {
        let res_type = reader.read_u8()?;
        let res_number = reader.read_u16_le()?;
        let packed_size = reader.read_u16_le()?;
        let unpacked_size = reader.read_u16_le()?;
        let compression_type = reader.read_u16_le()?;
        Ok(RawEntryHeader {
            res_type,
            res_number,
            packed_size,
            unpacked_size,
            compression_type,
        })
    }
}

pub struct RawContents {
    res_type: u8,
    res_number: u16,
    unpacked_size: u16,
    compression_type: u16,
    data: BlockSource,
}

impl std::fmt::Debug for RawContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawContents")
            .field("res_type", &self.res_type)
            .field("res_number", &self.res_number)
            .field("unpacked_size", &self.unpacked_size)
            .field("compression_type", &self.compression_type)
            .field("data", &self.data.size())
            .finish()
    }
}

enum HuffmanTableEntry<T> {
    Leaf(T),
    Branch(usize, usize),
}

struct HuffmanTable<T> {
    entries: Vec<HuffmanTableEntry<T>>,
}

impl<T> HuffmanTable<T> {
    fn lookup(&self, reader: &mut bitter::LittleEndianReader) -> io::Result<&T> {
        let mut pos = 0;
        loop {
            match &self.entries[pos] {
                HuffmanTableEntry::Leaf(value) => return Ok(value),
                HuffmanTableEntry::Branch(left, right) => {
                    let bit = reader
                        .read_bit()
                        .ok_or_else(|| io::Error::other("Failed to read bit"))?;
                    pos = if bit { *right } else { *left };
                }
            }
        }
    }
}

mod trees {
    use super::{HuffmanTable, HuffmanTableEntry};
    use std::sync::LazyLock;

    fn bn(_pos: usize, left: usize, right: usize) -> HuffmanTableEntry<u8> {
        HuffmanTableEntry::Branch(left, right)
    }

    fn ln(_pos: usize, value: u8) -> HuffmanTableEntry<u8> {
        HuffmanTableEntry::Leaf(value)
    }

    pub static LENGTH_TREE: LazyLock<HuffmanTable<u8>> = LazyLock::new(|| HuffmanTable {
        entries: vec![
            bn(0, 1, 2),
            bn(1, 3, 4),
            bn(2, 5, 6),
            bn(3, 7, 8),
            bn(4, 9, 10),
            bn(5, 11, 12),
            ln(6, 1),
            bn(7, 13, 14),
            bn(8, 15, 16),
            bn(9, 17, 18),
            ln(10, 3),
            ln(11, 2),
            ln(12, 0),
            bn(13, 19, 20),
            bn(14, 21, 22),
            bn(15, 23, 24),
            ln(16, 6),
            ln(17, 5),
            ln(18, 4),
            bn(19, 25, 26),
            bn(20, 27, 28),
            ln(21, 10),
            ln(22, 9),
            ln(23, 8),
            ln(24, 7),
            bn(25, 29, 30),
            ln(26, 13),
            ln(27, 12),
            ln(28, 11),
            ln(29, 15),
            ln(30, 14),
        ],
    });

    pub static DISTANCE_TREE: LazyLock<HuffmanTable<u8>> = LazyLock::new(|| {
        HuffmanTable {
            entries: vec![
                bn(0, 1, 2),
                bn(1, 3, 4),
                bn(2, 5, 6),
                //
                bn(3, 7, 8),
                bn(4, 9, 10),
                bn(5, 11, 12),
                ln(6, 0),
                bn(7, 13, 14),
                bn(8, 15, 16),
                bn(9, 17, 18),
                bn(10, 19, 20),
                bn(11, 21, 22),
                bn(12, 23, 24),
                //
                bn(13, 25, 26),
                bn(14, 27, 28),
                bn(15, 29, 30),
                bn(16, 31, 32),
                bn(17, 33, 34),
                bn(18, 35, 36),
                bn(19, 37, 38),
                bn(20, 39, 40),
                bn(21, 41, 42),
                bn(22, 43, 44),
                ln(23, 2),
                ln(24, 1),
                //
                bn(25, 45, 46),
                bn(26, 47, 48),
                bn(27, 49, 50),
                bn(28, 51, 52),
                bn(29, 53, 54),
                bn(30, 55, 56),
                bn(31, 57, 58),
                bn(32, 59, 60),
                bn(33, 61, 62),
                bn(34, 63, 64),
                bn(35, 65, 66),
                bn(36, 67, 68),
                bn(37, 69, 70),
                bn(38, 71, 72),
                bn(39, 73, 74),
                bn(40, 75, 76),
                ln(41, 6),
                ln(42, 5),
                ln(43, 4),
                ln(44, 3),
                //
                bn(45, 77, 78),
                bn(46, 79, 80),
                bn(47, 81, 82),
                bn(48, 83, 84),
                bn(49, 85, 86),
                bn(50, 87, 88),
                bn(51, 89, 90),
                bn(52, 91, 92),
                bn(53, 93, 94),
                bn(54, 95, 96),
                bn(55, 97, 98),
                bn(56, 99, 100),
                bn(57, 101, 102),
                bn(58, 103, 104),
                bn(59, 105, 106),
                bn(60, 107, 108),
                bn(61, 109, 110),
                ln(62, 21),
                ln(63, 20),
                ln(64, 19),
                ln(65, 18),
                ln(66, 17),
                ln(67, 16),
                ln(68, 15),
                ln(69, 14),
                ln(70, 13),
                ln(71, 12),
                ln(72, 11),
                ln(73, 10),
                ln(74, 9),
                ln(75, 8),
                ln(76, 7),
                //
                bn(77, 111, 112),
                bn(78, 113, 114),
                bn(79, 115, 116),
                bn(80, 117, 118),
                bn(81, 119, 120),
                bn(82, 121, 122),
                bn(83, 123, 124),
                bn(84, 125, 126),
                ln(85, 47),
                ln(86, 46),
                ln(87, 45),
                ln(88, 44),
                ln(89, 43),
                ln(90, 42),
                ln(91, 41),
                ln(92, 40),
                ln(93, 39),
                ln(94, 38),
                ln(95, 37),
                ln(96, 36),
                ln(97, 35),
                ln(98, 34),
                ln(99, 33),
                ln(100, 32),
                ln(101, 31),
                ln(102, 30),
                ln(103, 29),
                ln(104, 28),
                ln(105, 27),
                ln(106, 26),
                ln(107, 25),
                ln(108, 24),
                ln(109, 23),
                ln(110, 22),
                ln(111, 63),
                ln(112, 62),
                ln(113, 61),
                ln(114, 60),
                ln(115, 59),
                ln(116, 58),
                ln(117, 57),
                ln(118, 56),
                ln(119, 55),
                ln(120, 54),
                ln(121, 53),
                ln(122, 52),
                ln(123, 51),
                ln(124, 50),
                ln(125, 49),
                ln(126, 48),
            ],
        }
    });

    pub static ASCII_TREE: LazyLock<HuffmanTable<u8>> = LazyLock::new(|| {
        HuffmanTable {
            entries: vec![
                bn(0, 1, 2),
                bn(1, 3, 4),
                bn(2, 5, 6),
                bn(3, 7, 8),
                bn(4, 9, 10),
                bn(5, 11, 12),
                bn(6, 13, 14),
                bn(7, 15, 16),
                bn(8, 17, 18),
                bn(9, 19, 20),
                bn(10, 21, 22),
                bn(11, 23, 24),
                bn(12, 25, 26),
                bn(13, 27, 28),
                bn(14, 29, 30),
                bn(15, 31, 32),
                bn(16, 33, 34),
                bn(17, 35, 36),
                bn(18, 37, 38),
                bn(19, 39, 40),
                bn(20, 41, 42),
                bn(21, 43, 44),
                bn(22, 45, 46),
                bn(23, 47, 48),
                bn(24, 49, 50),
                bn(25, 51, 52),
                bn(26, 53, 54),
                bn(27, 55, 56),
                bn(28, 57, 58),
                bn(29, 59, 60),
                ln(30, 32),
                //
                bn(31, 61, 62),
                bn(32, 63, 64),
                bn(33, 65, 66),
                bn(34, 67, 68),
                bn(35, 69, 70),
                bn(36, 71, 72),
                bn(37, 73, 74),
                bn(38, 75, 76),
                bn(39, 77, 78),
                bn(40, 79, 80),
                bn(41, 81, 82),
                bn(42, 83, 84),
                bn(43, 85, 86),
                bn(44, 87, 88),
                bn(45, 89, 90),
                bn(46, 91, 92),
                bn(47, 93, 94),
                bn(48, 95, 96),
                bn(49, 97, 98),
                ln(50, 117),
                ln(51, 116),
                ln(52, 115),
                ln(53, 114),
                ln(54, 111),
                ln(55, 110),
                ln(56, 108),
                ln(57, 105),
                ln(58, 101),
                ln(59, 97),
                ln(60, 69),
                //
                bn(61, 99, 100),
                bn(62, 101, 102),
                bn(63, 103, 104),
                bn(64, 105, 106),
                bn(65, 107, 108),
                bn(66, 109, 110),
                bn(67, 111, 112),
                bn(68, 113, 114),
                bn(69, 115, 116),
                bn(70, 117, 118),
                bn(71, 119, 120),
                bn(72, 121, 122),
                bn(73, 123, 124),
                bn(74, 125, 126),
                bn(75, 127, 128),
                bn(76, 129, 130),
                bn(77, 131, 132),
                bn(78, 133, 134),
                ln(79, 112),
                ln(80, 109),
                ln(81, 104),
                ln(82, 103),
                ln(83, 102),
                ln(84, 100),
                ln(85, 99),
                ln(86, 98),
                ln(87, 84),
                ln(88, 83),
                ln(89, 82),
                ln(90, 79),
                ln(91, 78),
                ln(92, 76),
                ln(93, 73),
                ln(94, 68),
                ln(95, 67),
                ln(96, 65),
                ln(97, 49),
                ln(98, 45),
                //
                bn(99, 135, 136),
                bn(100, 137, 138),
                bn(101, 139, 140),
                bn(102, 141, 142),
                bn(103, 143, 144),
                bn(104, 145, 146),
                bn(105, 147, 148),
                bn(106, 149, 150),
                bn(107, 151, 152),
                bn(108, 153, 154),
                bn(109, 155, 156),
                bn(110, 157, 158),
                bn(111, 159, 160),
                bn(112, 161, 162),
                bn(113, 163, 164),
                ln(114, 119),
                ln(115, 107),
                ln(116, 85),
                ln(117, 80),
                ln(118, 77),
                ln(119, 70),
                ln(120, 66),
                ln(121, 61),
                ln(122, 56),
                ln(123, 55),
                ln(124, 53),
                ln(125, 52),
                ln(126, 51),
                ln(127, 50),
                ln(128, 48),
                ln(129, 46),
                ln(130, 44),
                ln(131, 41),
                ln(132, 40),
                ln(133, 13),
                ln(134, 10),
                //
                bn(135, 165, 166),
                bn(136, 167, 168),
                bn(137, 169, 170),
                bn(138, 171, 172),
                bn(139, 173, 174),
                bn(140, 175, 176),
                bn(141, 177, 178),
                bn(142, 179, 180),
                bn(143, 181, 182),
                bn(144, 183, 184),
                bn(145, 185, 186),
                bn(146, 187, 188),
                bn(147, 189, 190),
                bn(148, 191, 192),
                ln(149, 121),
                ln(150, 120),
                ln(151, 118),
                ln(152, 95),
                ln(153, 91),
                ln(154, 87),
                ln(155, 72),
                ln(156, 71),
                ln(157, 58),
                ln(158, 57),
                ln(159, 54),
                ln(160, 47),
                ln(161, 42),
                ln(162, 39),
                ln(163, 34),
                ln(164, 9),
                //
                bn(165, 193, 194),
                bn(166, 195, 196),
                bn(167, 197, 198),
                bn(168, 199, 200),
                bn(169, 201, 202),
                bn(170, 203, 204),
                bn(171, 205, 206),
                bn(172, 207, 208),
                bn(173, 209, 210),
                bn(174, 211, 212),
                bn(175, 213, 214),
                bn(176, 215, 216),
                bn(177, 217, 218),
                bn(178, 219, 220),
                bn(179, 221, 222),
                bn(180, 223, 224),
                bn(181, 225, 226),
                bn(182, 227, 228),
                bn(183, 229, 230),
                bn(184, 231, 232),
                bn(185, 233, 234),
                ln(186, 93),
                ln(187, 89),
                ln(188, 88),
                ln(189, 86),
                ln(190, 75),
                ln(191, 62),
                ln(192, 43),
                //
                bn(193, 235, 236),
                bn(194, 237, 238),
                bn(195, 239, 240),
                bn(196, 241, 242),
                bn(197, 243, 244),
                bn(198, 245, 246),
                bn(199, 247, 248),
                bn(200, 249, 250),
                bn(201, 251, 252),
                bn(202, 253, 254),
                bn(203, 255, 256),
                bn(204, 257, 258),
                bn(205, 259, 260),
                bn(206, 261, 262),
                bn(207, 263, 264),
                bn(208, 265, 266),
                bn(209, 267, 268),
                bn(210, 269, 270),
                bn(211, 271, 272),
                bn(212, 273, 274),
                bn(213, 275, 276),
                bn(214, 277, 278),
                bn(215, 279, 280),
                bn(216, 281, 282),
                bn(217, 283, 284),
                bn(218, 285, 286),
                bn(219, 287, 288),
                bn(220, 289, 290),
                bn(221, 291, 292),
                bn(222, 293, 294),
                bn(223, 295, 296),
                bn(224, 297, 298),
                bn(225, 299, 300),
                bn(226, 301, 302),
                bn(227, 303, 304),
                bn(228, 305, 306),
                bn(229, 307, 308),
                ln(230, 122),
                ln(231, 113),
                ln(232, 38),
                ln(233, 36),
                ln(234, 33),
                //
                bn(235, 309, 310),
                bn(236, 311, 312),
                bn(237, 313, 314),
                bn(238, 315, 316),
                bn(239, 317, 318),
                bn(240, 319, 320),
                bn(241, 321, 322),
                bn(242, 323, 324),
                bn(243, 325, 326),
                bn(244, 327, 328),
                bn(245, 329, 330),
                bn(246, 331, 332),
                bn(247, 333, 334),
                bn(248, 335, 336),
                bn(249, 337, 338),
                bn(250, 339, 340),
                bn(251, 341, 342),
                bn(252, 343, 344),
                bn(253, 345, 346),
                bn(254, 347, 348),
                bn(255, 349, 350),
                bn(256, 351, 352),
                bn(257, 353, 354),
                bn(258, 355, 356),
                bn(259, 357, 358),
                bn(260, 359, 360),
                bn(261, 361, 362),
                bn(262, 363, 364),
                bn(263, 365, 366),
                bn(264, 367, 368),
                bn(265, 369, 370),
                bn(266, 371, 372),
                bn(267, 373, 374),
                bn(268, 375, 376),
                bn(269, 377, 378),
                bn(270, 379, 380),
                bn(271, 381, 382),
                bn(272, 383, 384),
                bn(273, 385, 386),
                bn(274, 387, 388),
                bn(275, 389, 390),
                bn(276, 391, 392),
                bn(277, 393, 394),
                bn(278, 395, 396),
                bn(279, 397, 398),
                bn(280, 399, 400),
                bn(281, 401, 402),
                bn(282, 403, 404),
                bn(283, 405, 406),
                bn(284, 407, 408),
                bn(285, 409, 410),
                bn(286, 411, 412),
                bn(287, 413, 414),
                bn(288, 415, 416),
                bn(289, 417, 418),
                bn(290, 419, 420),
                bn(291, 421, 422),
                bn(292, 423, 424),
                bn(293, 425, 426),
                bn(294, 427, 428),
                bn(295, 429, 430),
                bn(296, 431, 432),
                bn(297, 433, 434),
                bn(298, 435, 436),
                ln(299, 124),
                ln(300, 123),
                ln(301, 106),
                ln(302, 92),
                ln(303, 90),
                ln(304, 81),
                ln(305, 74),
                ln(306, 63),
                ln(307, 60),
                ln(308, 0),
                //
                bn(309, 437, 438),
                bn(310, 439, 440),
                bn(311, 441, 442),
                bn(312, 443, 444),
                bn(313, 445, 446),
                bn(314, 447, 448),
                bn(315, 449, 450),
                bn(316, 451, 452),
                bn(317, 453, 454),
                bn(318, 455, 456),
                bn(319, 457, 458),
                bn(320, 459, 460),
                bn(321, 461, 462),
                bn(322, 463, 464),
                bn(323, 465, 466),
                bn(324, 467, 468),
                bn(325, 469, 470),
                bn(326, 471, 472),
                bn(327, 473, 474),
                bn(328, 475, 476),
                bn(329, 477, 478),
                bn(330, 479, 480),
                bn(331, 481, 482),
                bn(332, 483, 484),
                bn(333, 485, 486),
                bn(334, 487, 488),
                bn(335, 489, 490),
                bn(336, 491, 492),
                bn(337, 493, 494),
                bn(338, 495, 496),
                bn(339, 497, 498),
                bn(340, 499, 500),
                bn(341, 501, 502),
                bn(342, 503, 504),
                bn(343, 505, 506),
                bn(344, 507, 508),
                bn(345, 509, 510),
                ln(346, 244),
                ln(347, 243),
                ln(348, 242),
                ln(349, 238),
                ln(350, 233),
                ln(351, 229),
                ln(352, 225),
                ln(353, 223),
                ln(354, 222),
                ln(355, 221),
                ln(356, 220),
                ln(357, 219),
                ln(358, 218),
                ln(359, 217),
                ln(360, 216),
                ln(361, 215),
                ln(362, 214),
                ln(363, 213),
                ln(364, 212),
                ln(365, 211),
                ln(366, 210),
                ln(367, 209),
                ln(368, 208),
                ln(369, 207),
                ln(370, 206),
                ln(371, 205),
                ln(372, 204),
                ln(373, 203),
                ln(374, 202),
                ln(375, 201),
                ln(376, 200),
                ln(377, 199),
                ln(378, 198),
                ln(379, 197),
                ln(380, 196),
                ln(381, 195),
                ln(382, 194),
                ln(383, 193),
                ln(384, 192),
                ln(385, 191),
                ln(386, 190),
                ln(387, 189),
                ln(388, 188),
                ln(389, 187),
                ln(390, 186),
                ln(391, 185),
                ln(392, 184),
                ln(393, 183),
                ln(394, 182),
                ln(395, 181),
                ln(396, 180),
                ln(397, 179),
                ln(398, 178),
                ln(399, 177),
                ln(400, 176),
                ln(401, 127),
                ln(402, 126),
                ln(403, 125),
                ln(404, 96),
                ln(405, 94),
                ln(406, 64),
                ln(407, 59),
                ln(408, 37),
                ln(409, 35),
                ln(410, 31),
                ln(411, 30),
                ln(412, 29),
                ln(413, 28),
                ln(414, 27),
                ln(415, 25),
                ln(416, 24),
                ln(417, 23),
                ln(418, 22),
                ln(419, 21),
                ln(420, 20),
                ln(421, 19),
                ln(422, 18),
                ln(423, 17),
                ln(424, 16),
                ln(425, 15),
                ln(426, 14),
                ln(427, 12),
                ln(428, 11),
                ln(429, 8),
                ln(430, 7),
                ln(431, 6),
                ln(432, 5),
                ln(433, 4),
                ln(434, 3),
                ln(435, 2),
                ln(436, 1),
                ln(437, 255),
                ln(438, 254),
                ln(439, 253),
                ln(440, 252),
                ln(441, 251),
                ln(442, 250),
                ln(443, 249),
                ln(444, 248),
                ln(445, 247),
                ln(446, 246),
                ln(447, 245),
                ln(448, 241),
                ln(449, 240),
                ln(450, 239),
                ln(451, 237),
                ln(452, 236),
                ln(453, 235),
                ln(454, 234),
                ln(455, 232),
                ln(456, 231),
                ln(457, 230),
                ln(458, 228),
                ln(459, 227),
                ln(460, 226),
                ln(461, 224),
                ln(462, 175),
                ln(463, 174),
                ln(464, 173),
                ln(465, 172),
                ln(466, 171),
                ln(467, 170),
                ln(468, 169),
                ln(469, 168),
                ln(470, 167),
                ln(471, 166),
                ln(472, 165),
                ln(473, 164),
                ln(474, 163),
                ln(475, 162),
                ln(476, 161),
                ln(477, 160),
                ln(478, 159),
                ln(479, 158),
                ln(480, 157),
                ln(481, 156),
                ln(482, 155),
                ln(483, 154),
                ln(484, 153),
                ln(485, 152),
                ln(486, 151),
                ln(487, 150),
                ln(488, 149),
                ln(489, 148),
                ln(490, 147),
                ln(491, 146),
                ln(492, 145),
                ln(493, 144),
                ln(494, 143),
                ln(495, 142),
                ln(496, 141),
                ln(497, 140),
                ln(498, 139),
                ln(499, 138),
                ln(500, 137),
                ln(501, 136),
                ln(502, 135),
                ln(503, 134),
                ln(504, 133),
                ln(505, 132),
                ln(506, 131),
                ln(507, 130),
                ln(508, 129),
                ln(509, 128),
                ln(510, 26),
            ],
        }
    });
}

use trees::{ASCII_TREE, DISTANCE_TREE, LENGTH_TREE};

pub fn decompress_dcl(input: &[u8], output: &mut [u8]) -> io::Result<()> {
    // This follows the implementation from ScummVM, in DecompressorDCL::unpack()
    let mut reader = bitter::LittleEndianReader::new(input);
    let Some(mode) = reader.read_u8() else {
        return Err(io::Error::other("Failed to read DCL mode"));
    };
    let Some(dict_type) = reader.read_u8() else {
        return Err(io::Error::other("Failed to read DCL dictionary type"));
    };

    if mode != 0 && mode != 1 {
        return Err(io::Error::other(format!("Unsupported DCL mode: {}", mode)));
    }

    let dict_size = match dict_type {
        4 => 1024,
        5 => 2048,
        6 => 4096,
        _ => {
            return Err(io::Error::other(format!(
                "Unsupported DCL dictionary type: {}",
                dict_type
            )))
        }
    };
    let dict_mask: u32 = dict_size - 1;
    let mut dict = vec![0u8; dict_size as usize];
    let mut dict_pos: u32 = 0;
    let mut bytes_written: u32 = 0;

    loop {
        let should_decode_entry = reader
            .read_bit()
            .ok_or_else(|| io::Error::other("Failed to read DCL entry type"))?;
        if should_decode_entry {
            let length_code = *LENGTH_TREE.lookup(&mut reader)?;
            let token_length = if length_code < 8 {
                (length_code + 2) as u32
            } else {
                let num_bits = (length_code - 7) as u32;
                let extra_bits: u32 = reader
                    .read_bits(num_bits)
                    .ok_or_else(|| io::Error::other("Failed to read DCL extra length bits"))?
                    .try_into()
                    .unwrap();

                8 + (1 << num_bits) + extra_bits
            };

            if token_length == 519 {
                break;
            }

            let distance_code = *DISTANCE_TREE.lookup(&mut reader)? as u32;
            let token_offset: u32 =
                1 + if token_length == 2 {
                    distance_code << 2
                        | reader.read_bits(2).ok_or_else(|| {
                            io::Error::other("Failed to read DCL extra distance bits")
                        })? as u32
                } else {
                    distance_code << dict_type
                        | reader.read_bits(dict_type as u32).ok_or_else(|| {
                            io::Error::other("Failed to read DCL extra distance bits")
                        })? as u32
                };
            if token_length + bytes_written > output.len() as u32 {
                return Err(io::Error::other(
                    "DCL token length exceeds output buffer size",
                ));
            }
            if bytes_written < token_offset {
                return Err(io::Error::other("DCL token offset exceeds bytes written"));
            }

            dbg!(dict_pos, token_offset, token_length);

            let base_index = (dict_pos.wrapping_sub(token_offset)) & dict_mask;
            let mut curr_index = base_index;
            let mut next_index = dict_pos;

            for _ in 0..token_length {
                let curr_byte = dict[curr_index as usize];
                output[bytes_written as usize] = curr_byte;
                bytes_written += 1;
                dict[next_index as usize] = curr_byte;
                next_index = (next_index + 1) & dict_mask;
                curr_index = (curr_index + 1) & dict_mask;
                if curr_index == dict_pos {
                    curr_index = base_index;
                }

                if next_index == dict_size {
                    next_index = 0;
                }
                dict_pos = next_index;
            }
        } else {
            let value = if mode == 1 {
                *ASCII_TREE.lookup(&mut reader)?
            } else {
                reader
                    .read_u8()
                    .ok_or_else(|| io::Error::other("Failed to read DCL byte"))?
            };
            output[bytes_written as usize] = value;
            bytes_written += 1;
            dict[dict_pos as usize] = value;
            dict_pos += 1;
            if dict_pos >= dict_size {
                dict_pos = 0;
            }
        }
    }

    if bytes_written != output.len() as u32 {
        return Err(io::Error::other("DCL output buffer not fully written"));
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Contents {
    res_type: u8,
    res_number: u16,
    data: Block,
}

impl Contents {
    pub fn data(&self) -> &Block {
        &self.data
    }
}

impl TryFrom<RawContents> for Contents {
    type Error = io::Error;

    fn try_from(raw_contents: RawContents) -> Result<Self, Self::Error> {
        let decompressed_data = match raw_contents.compression_type {
            0 => {
                assert_eq!(raw_contents.data.size(), raw_contents.unpacked_size as u64);
                raw_contents.data.open()?
            }
            18 => {
                let compressed_data = raw_contents.data.open()?.read_all()?;
                let mut decompressed_data = vec![0; raw_contents.unpacked_size as usize];
                decompress_dcl(&compressed_data, &mut decompressed_data)?;
                Block::from_vec(decompressed_data)
            }
            _ => {
                return Err(io::Error::other(format!(
                    "Unsupported compression type: {}",
                    raw_contents.compression_type
                )));
            }
        };

        Ok(Contents {
            res_type: raw_contents.res_type,
            res_number: raw_contents.res_number,
            data: decompressed_data,
        })
    }
}

pub struct DataFile {
    data: BlockSource,
}

impl DataFile {
    pub fn new(data: BlockSource) -> DataFile {
        DataFile { data }
    }

    pub fn read_raw_contents(&self, location: &ResourceLocation) -> io::Result<RawContents> {
        let (header, rest) =
            RawEntryHeader::from_block_source(&self.data.subblock(location.file_offset as u64..))?;
        let resource_block = rest.subblock(..header.packed_size as u64);
        assert_eq!(resource_block.size(), header.packed_size as u64);
        Ok(RawContents {
            res_type: header.res_type,
            res_number: header.res_number,
            unpacked_size: header.unpacked_size,
            compression_type: header.compression_type,
            data: resource_block,
        })
    }

    pub fn read_contents(&self, location: &ResourceLocation) -> io::Result<Contents> {
        let raw_contents = self.read_raw_contents(location)?;
        raw_contents.try_into()
    }
}
