module A::B
  module C
    class D
      class E::F
      ^^^^^^^^^^ Missing top-level documentation comment for `class A::B::C::D::E::F`.
        def method
        end
      end
    end
  end
end
