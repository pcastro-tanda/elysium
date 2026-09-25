class A
  private
  def blah
  end
  begin
    def method1
    end
    public
    public
    ^^^^^^ Useless `public` access modifier.
    def method2
    end
  end
end
