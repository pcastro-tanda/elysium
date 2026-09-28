Dir.glob(
  Rails.root.join('./lib/**/*.rb'),
  File::FNM_DOTMATCH,
  &method(:require)
)
