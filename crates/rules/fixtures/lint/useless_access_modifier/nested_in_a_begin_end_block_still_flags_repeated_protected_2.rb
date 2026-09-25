class A
  private
  def blah
  end
  begin
    def method1
    end
    protected
    protected
    ^^^^^^^^^ Useless `protected` access modifier.
    def method2
    end
  end
end
