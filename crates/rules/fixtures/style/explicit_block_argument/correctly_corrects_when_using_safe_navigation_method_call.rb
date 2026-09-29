def do_something
  array&.each do |row|
  ^^^^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
    yield row
  end
end
