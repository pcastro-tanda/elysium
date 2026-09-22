case sexp.loc.keyword.source
in 'if'     then cond, body, _else = *sexp
in 'unless' then cond, _else, body = *sexp
else             cond, body = *sexp
end
