class SomeClass
  included do
    private
    private
    ^^^^^^^ Useless `private` access modifier.
    def foo; end
  end
  private
  private
  ^^^^^^^ Useless `private` access modifier.
  def bar; end
end
