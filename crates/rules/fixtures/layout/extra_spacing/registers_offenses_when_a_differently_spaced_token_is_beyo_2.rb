foo(:a)   { bar }
       ^^ Unnecessary spacing detected.
foo(:b)   { bar }
       ^^ Unnecessary spacing detected.
def unrelated
  bar
end
foo(:abc) { bar }
