class Foo
  def do_something; end
  CONST = [*foo].freeze
  ^^^^^^^^^^^^^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
  include M
  ^^^^^^^^^ `module_inclusion` is supposed to appear before `public_methods`.
end
