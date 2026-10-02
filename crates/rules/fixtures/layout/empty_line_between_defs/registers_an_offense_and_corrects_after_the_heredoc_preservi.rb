def foo = <<~TEXT
  hello
TEXT
def bar
^^^^^^^ Expected 1 empty line between method definitions; found 0.
  y
end
