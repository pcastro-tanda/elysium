class Foo
  def do_something; end

  private

  CONST = 1
  ^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
end
