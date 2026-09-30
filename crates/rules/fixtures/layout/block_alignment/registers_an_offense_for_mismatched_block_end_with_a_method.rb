parser.children << lambda do |token|
  token << 1
  end
  ^^^ `end` at 3, 2 is not aligned with `parser.children << lambda do |token|` at 1, 0.
