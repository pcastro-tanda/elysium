format(<<-HEREDOC, foo: bar)
foo %<named>f + bar %{template}
                    ^^^^^^^^^^^ Prefer annotated tokens (like `%<foo>s`) over template tokens (like `%{foo}`).
HEREDOC
