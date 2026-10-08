class Foo
  def self.do_something; end

  def initialize; end

  include M
  ^^^^^^^^^ `module_inclusion` is supposed to appear before `initializer`.
  CONST = 1
  ^^^^^^^^^ `constants` is supposed to appear before `initializer`.
end
