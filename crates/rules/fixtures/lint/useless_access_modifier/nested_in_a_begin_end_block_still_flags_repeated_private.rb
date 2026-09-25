module A
  protected
  def blah
  end
  begin
    def method1
    end
    private
    private
    ^^^^^^^ Useless `private` access modifier.
    def method2
    end
  end
end
