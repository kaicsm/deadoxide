//! zombie.iso.worldgen.utils.AreaSquareVar: the running bounds of the squares of one generated area.

/// zombie.iso.worldgen.utils.AreaSquareVar: four private ints, `xmin`/`ymin` at Integer.MAX_VALUE and
/// `xmax`/`ymax` at -2147483647 (-Integer.MAX_VALUE, one above Integer.MIN_VALUE) so that any first square
/// tightens them.
#[derive(Clone, Debug)]
pub struct AreaSquareVar {
    xmin: i32,
    xmax: i32,
    ymin: i32,
    ymax: i32,
}

impl Default for AreaSquareVar {
    fn default() -> Self {
        Self::new()
    }
}

impl AreaSquareVar {
    /// zombie.iso.worldgen.utils.AreaSquareVar.AreaSquareVar(): the implicit constructor, fields at their
    /// declared initializers.
    pub fn new() -> Self {
        AreaSquareVar {
            xmin: i32::MAX,
            xmax: -2147483647,
            ymin: i32::MAX,
            ymax: -2147483647,
        }
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.getXmin
    pub fn get_xmin(&self) -> i32 {
        self.xmin
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.setXmin
    pub fn set_xmin(&mut self, xmin: i32) {
        self.xmin = xmin;
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.getXmax
    pub fn get_xmax(&self) -> i32 {
        self.xmax
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.setXmax
    pub fn set_xmax(&mut self, xmax: i32) {
        self.xmax = xmax;
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.getYmin
    pub fn get_ymin(&self) -> i32 {
        self.ymin
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.setYmin
    pub fn set_ymin(&mut self, ymin: i32) {
        self.ymin = ymin;
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.getYmax
    pub fn get_ymax(&self) -> i32 {
        self.ymax
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.setYmax
    pub fn set_ymax(&mut self, ymax: i32) {
        self.ymax = ymax;
    }

    /// zombie.iso.worldgen.utils.AreaSquareVar.toString — note the class name in the text is "CellSquare",
    /// not AreaSquareVar; kept as the original prints it.
    pub fn to_string(&self) -> String {
        format!(
            "CellSquare{{xmin={}, xmax={}, ymin={}, ymax={}}}",
            self.xmin, self.xmax, self.ymin, self.ymax
        )
    }
}
