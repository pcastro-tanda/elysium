def bar
end

attribute :foo, :string, default: bar
                                  ^^^ Pass method in a block to `:default` option.
