attribute :foo, :string, limit: 1,
                         default: -> { Foo.bar }
