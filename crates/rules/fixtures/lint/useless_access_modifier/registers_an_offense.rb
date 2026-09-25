class SomeClass
  included do
    private
    def foo; end
  end
  private
  ^^^^^^^ Useless `private` access modifier.
  def bar; end
end
