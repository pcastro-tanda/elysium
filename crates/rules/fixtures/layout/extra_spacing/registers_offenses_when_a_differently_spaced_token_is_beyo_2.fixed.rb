foo(:a) { bar }
foo(:b) { bar }
def unrelated
  bar
end
foo(:abc) { bar }
