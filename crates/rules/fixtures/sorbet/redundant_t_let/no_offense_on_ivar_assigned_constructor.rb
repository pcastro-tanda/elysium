sig { params(path: String).void }
def initialize(path)
  @path = T.let(Pathname.new(path), Pathname)
end
