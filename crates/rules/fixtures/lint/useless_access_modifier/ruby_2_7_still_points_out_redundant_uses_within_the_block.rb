class SomeClass
  concerning :SecondThing do
    p _1
    def omg
    end
    private
    def method
    end
    private
    ^^^^^^^ Useless `private` access modifier.
    def another_method
    end
  end
 end
