# legs

A TUI for viewing R data

## Installation

You can install the development version of legs from
[GitHub](https://github.com/) with:

``` r

# install.packages("pak")
pak::pak("ryanzomorrodi/legs")
```

## Usage

`legs` is a Terminal User Interface (TUI) for viewing your data in R.
`legs` is capable of viewing `data.frame`s, `matrix`s, `lists`, and
atomic `vectors`. Just call
[`legs::view()`](https://ryanzomorrodi.github.io/legs/reference/view.md)
on your object to open the viewer in your terminal.

![](reference/figures/explore.gif)

Navigate the terminal with the following key bindings:

| Key | Action |
|----|----|
| `hjkl` (or `← ↓ ↑ →`) | Scroll one row or column in the given direction |
| `HJKL` (or `Shift + ← ↓ ↑ →`) | Scroll one window in the given direction |
| `$` | Scroll to last column |
| `^` | Scroll to first column |
| `G` | Scroll to bottom |
| `<n>G` | Scroll to `<n>` row |
| `g` | Scroll to top |
| `Enter` | View the cell highlighted |
| `esc` | View the parent data structure |
| `t` | Toggle cell width truncation |
| `q` | Exit |

All scroll movement key bindings can also be prefixed with a number to
perform it `<n>` times. For example, pressing `25h` scrolls down 25
rows.

[`legs::view()`](https://ryanzomorrodi.github.io/legs/reference/view.md)
also prints the last frame shown and silently returns the last item
viewed. Meaning you can use it to interactive pluck deeply nested data.

![](reference/figures/select_data.gif)
