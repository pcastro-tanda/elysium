attribute :foo, :string, limit: 1,
                         default: Foo.bar
                                  ^^^^^^^ Pass method in a block to `:default` option.
