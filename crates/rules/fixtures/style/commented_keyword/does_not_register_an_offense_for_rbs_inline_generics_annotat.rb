class X < Y #[String]
end
class A < B::C #[String]
end
class A < B::C::D #[String]
end
class A::B < C #[String]
end
class A::B::C < D #[String]
end
