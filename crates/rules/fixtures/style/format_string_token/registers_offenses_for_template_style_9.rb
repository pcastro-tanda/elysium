format(<<-HEREDOC, foo: bar)
foo %<named>d + bar %{template}
                    ^^^^^^^^^^^ Prefer annotated tokens (like `%<foo>s`) over template tokens (like `%{foo}`).
HEREDOC
