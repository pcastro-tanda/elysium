class Foo
  def self::bar
          ^^ Do not use `::` for defining class methods.
    something
  end
end
