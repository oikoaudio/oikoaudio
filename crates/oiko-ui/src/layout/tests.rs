use super::*;
#[test]
fn rows_share_edges_and_fill_the_available_width() {
    let grid = Columns::equal(12.0, 360.0, 3, GAP);
    for column in 0..3 {
        let header = grid.cell(column, 0.0, 20.0);
        let field = grid.cell(column, 26.0, 28.0);
        assert_eq!(header.x_range(), field.x_range());
    }
    assert_eq!(grid.cell(2, 0.0, 20.0).right(), 372.0);
}
