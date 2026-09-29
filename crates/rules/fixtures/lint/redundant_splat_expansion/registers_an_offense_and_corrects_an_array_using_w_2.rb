case foo
when *%W(#{first} second)
     ^^^^^^^^^^^^^^^^^^^^ Replace splat expansion with comma separated values.
  bar
end
