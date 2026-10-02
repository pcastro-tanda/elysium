class Foo
  def do_something; end
  include M
  ^^^^^^^^^ `module_inclusion` is supposed to appear before `public_methods`.
  CONST = 1
  ^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
end
