# Invoke legs Data Viewer

Invoke the legs terminal user interface (tui) to interactively explore R
data.

## Usage

``` r
view(x)
```

## Arguments

- x:

  A data.frame, matrix, list, or atomic vector

## Value

The last viewed item

## Examples

``` r
if (interactive()) {
  df <- data.frame(x = 1:10, y = LETTERS[1:10])
  view(df)
  view(as.matrix(df))
  view(as.list(df))
  view(df$x)
}
```
