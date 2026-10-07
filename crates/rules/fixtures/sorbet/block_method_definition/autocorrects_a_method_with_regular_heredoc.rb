            yielding_method do
              def example
              ^^^^^^^^^^^ Sorbet/BlockMethodDefinition: Do not define methods in blocks (use `define_method` as a workaround).
                <<EOF
This is a test
with multiple lines
EOF
              end
            end
