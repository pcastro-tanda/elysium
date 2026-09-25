class SomeClass
  included do
    private
    private
    ^^^^^^^ Useless `private` access modifier.
    def foo; end
  end
  private
  ^^^^^^^ Useless `private` access modifier.
  private
  ^^^^^^^ Useless `private` access modifier.
  def bar; end
end
