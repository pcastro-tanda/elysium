sig do
  params(name: String)
    .returns(String)
     ^^^^^^^^^^^^^^^ Setter methods must declare a `void` return type.
end
def name=(name); end
