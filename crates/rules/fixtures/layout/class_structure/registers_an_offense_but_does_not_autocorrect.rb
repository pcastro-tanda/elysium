class Person
  def name; end

  foo = 5
  LIMIT = foo + 1
  ^^^^^^^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
end
