class Foo
  def name; end

  LIMIT = 10
  ^^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
  CONST = 'wrong place'.freeze
  RECURSIVE_BASIC_LITERALS_CONST = [1, 2].freeze
  DYNAMIC_CONST = foo.freeze
end
