#' @title Invoke legs Data Viewer
#' @description Invoke the legs terminal user interface (tui) to interactively explore R data.
#' @param x A data.frame, matrix, list, or atomic vector
#' @return The last viewed item
#' @examples
#' if (interactive()) {
#'   df <- data.frame(x = 1:10, y = LETTERS[1:10])
#'   view(df)
#'   view(as.matrix(df))
#'   view(as.list(df))
#'   view(df$x)
#' }
#'
#' @export
view <- function(x) {
  visible_view(x) |> invisible()
}
