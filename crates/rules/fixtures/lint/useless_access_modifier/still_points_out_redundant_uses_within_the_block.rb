class SomeClass
  concerning :FirstThing do
    def foo
    end
    private

    def method
    end
  end

  concerning :SecondThing do
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
