//! zombie.iso.worldgen.zombie.ZombieVoronoiReader: the `zombie_voronoi` Lua table into its entries
//! (the original's stateless class - the port keeps the free function).

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{
    cast_double, cast_string, cast_table, rstr, Object, R, TableRef, Throwable,
};

use super::zombie_voronoi_entry::ZombieVoronoiEntry;

/// ZombieVoronoiReader.getEntries(KahluaTable): every value of the table, in the Kahlua iteration
/// order - each cast `(KahluaTable)` and read through the four loads - into one entry per value (the
/// record's String overload, so a bad `closest` is `ClosestSelectionType.valueOf`'s
/// `java.lang.IllegalArgumentException`).
pub fn get_entries(main_table: &TableRef) -> R<Vec<ZombieVoronoiEntry>> {
    let mut zombie_voronoi_entries = Vec::new();
    for (_, value) in KahluaTableImpl::iterate_snapshot(main_table) {
        // (KahluaTable) iterZombieVoronoi.getValue(): any non-table value is the ClassCastException
        // (a null value, which the iterator never yields, would pass the cast and NPE on the first
        // rawget of the original)
        let table =
            cast_table(&value)?.ok_or_else(|| Throwable::npe("zombie_voronoi entry table"))?;
        let points = load_integer(&KahluaTableImpl::rawget(&table, &Object::str("points"))?, 1)?;
        let closest = load_string(
            &KahluaTableImpl::rawget(&table, &Object::str("closest"))?,
            "SECOND_MINUS_FIRST",
        )?;
        let scale = load_double(&KahluaTableImpl::rawget(&table, &Object::str("scale"))?, 16.0)?;
        let cutoff = load_double(&KahluaTableImpl::rawget(&table, &Object::str("cutoff"))?, 0.2)?;
        zombie_voronoi_entries.push(
            ZombieVoronoiEntry::from_name(points, &closest, scale, cutoff)
                .map_err(illegal_argument)?,
        );
    }
    Ok(zombie_voronoi_entries)
}

/// ZombieVoronoiReader.loadInteger(Object, int): null the default, else `((Double)o).intValue()` -
/// the double narrowed to the int (NaN to 0, the infinities saturated, the rest truncated), which is
/// Rust's `as i32`. Anything else is the `(Double)` ClassCastException.
fn load_integer(object: &Object, default_value: i32) -> R<i32> {
    Ok(cast_double(object)?
        .map(|d| d as i32)
        .unwrap_or(default_value))
}

/// ZombieVoronoiReader.loadDouble(Object, Double): null the default, else the `(Double)o` cast.
fn load_double(object: &Object, default_value: f64) -> R<f64> {
    Ok(cast_double(object)?.unwrap_or(default_value))
}

/// ZombieVoronoiReader.loadString(Object, String): null the default, else the `(String)o` cast.
fn load_string(object: &Object, default_value: &str) -> R<String> {
    Ok(match cast_string(object)? {
        Some(s) => rstr(&s),
        None => default_value.to_string(),
    })
}

/// The `java.lang.IllegalArgumentException` `ClosestSelectionType.valueOf` raised (its text is what
/// the entry String overload's `Err` carries) as the port's `Throwable`: the class and the message
/// the text holds.
fn illegal_argument(message: String) -> Throwable {
    let message = match message.strip_prefix("java.lang.IllegalArgumentException: ") {
        Some(m) => m.to_string(),
        None => message,
    };
    Throwable::Java {
        class: "java.lang.IllegalArgumentException",
        message: Some(message),
    }
}
