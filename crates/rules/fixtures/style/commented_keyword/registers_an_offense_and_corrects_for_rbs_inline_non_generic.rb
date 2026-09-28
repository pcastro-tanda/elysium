class X #[String]
        ^^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class X < Y #[String
            ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class X < Y #String]
            ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class X < Y # String]
            ^^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class X < Y #String ]
            ^^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class A < B::C #String]
               ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class A < B::C::D #String]
                  ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class A::B < C #String]
               ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
class A::B::C < D #String]
                  ^^^^^^^^ Do not place comments on the same line as the `class` keyword.
end
