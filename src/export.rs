use crate::interp::XYPoint;
use crate::types::{AxisUnit, AxisValue};
use rust_xlsxwriter::{Format, Workbook, XlsxError};

pub fn export_to_csv(
    path: &std::path::Path,
    x_unit: AxisUnit,
    y_unit: AxisUnit,
    data: &[XYPoint],
) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(["x", "y"])?;
    for p in data {
        let xv = AxisValue::from_scalar_seconds(x_unit, p.x);
        let yv = AxisValue::from_scalar_seconds(y_unit, p.y);
        wtr.write_record([xv.format(), yv.format()])?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn export_to_xlsx(
    path: &std::path::Path,
    x_unit: AxisUnit,
    y_unit: AxisUnit,
    data: &[XYPoint],
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet.write_string(0, 0, "x")?;
    worksheet.write_string(0, 1, "y")?;

    // Simple formatting to auto-size columns a bit
    let num_format = Format::new().set_num_format("0.0000");

    for (i, p) in data.iter().enumerate() {
        let row = (i + 1) as u32;
        match x_unit {
            AxisUnit::Float => {
                worksheet.write_number_with_format(row, 0, p.x, &num_format)?;
            }
            AxisUnit::DateTime => {
                let xv = AxisValue::from_scalar_seconds(x_unit, p.x);
                worksheet.write_string(row, 0, xv.format())?;
            }
        }

        match y_unit {
            AxisUnit::Float => {
                worksheet.write_number_with_format(row, 1, p.y, &num_format)?;
            }
            AxisUnit::DateTime => {
                let yv = AxisValue::from_scalar_seconds(y_unit, p.y);
                worksheet.write_string(row, 1, yv.format())?;
            }
        }
    }

    workbook.save(path)
}
