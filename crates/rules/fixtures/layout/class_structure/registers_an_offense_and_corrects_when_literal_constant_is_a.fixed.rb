class Foo
  LIMIT = 10
  CONST = 'wrong place'.freeze
  RECURSIVE_BASIC_LITERALS_CONST = [1, 2].freeze
  def name; end

  DYNAMIC_CONST = foo.freeze
end
