case sexp.loc.keyword.source
when 'if'     then cond, body, _else = *sexp
when 'unless' then cond, _else, body = *sexp
else               cond, body = *sexp
end
