class Foo
  private

  def do_internal_work; end

  public

  attr_reader :foo
  ^^^^^^^^^^^^^^^^ `attribute_macros` is supposed to appear before `private_methods`.
end
