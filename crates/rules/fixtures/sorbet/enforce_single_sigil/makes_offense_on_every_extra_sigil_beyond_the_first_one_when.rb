# typed: true
# typed: false
^^^^^^^^^^^^^^ Sorbet/EnforceSingleSigil: Files must only contain one sigil
# frozen_string_literal: true
# hello there
# typed: true
^^^^^^^^^^^^^ Sorbet/EnforceSingleSigil: Files must only contain one sigil
class Foo; end
