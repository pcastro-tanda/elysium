format(<<-HEREDOC, foo: bar)
foo %{template} + bar %<named>s
                      ^^^^^^^^^ Prefer template tokens (like `%{foo}`) over annotated tokens (like `%<foo>s`).
HEREDOC
