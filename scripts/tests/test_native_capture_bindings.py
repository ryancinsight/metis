"""Compile actual source scopes without invoking or substituting Win32 calls."""
from __future__ import annotations

import ast
import pathlib
import symtable
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]


class NativeCaptureBindingsTests(unittest.TestCase):
    def test_pointer_guards_resolve_their_real_global_dependencies(self) -> None:
        path = SCRIPTS / "native_input/windows/pointer.py"
        table = symtable.symtable(path.read_text(), str(path), "exec")
        scopes = {child.get_name(): child for child in table.get_children()}
        for function, names in (
            ("guard_surface_visible", ("window", "coverer", "geometry")),
            ("guard_input_point", ("coverer",)),
        ):
            for name in names:
                with self.subTest(function=function, dependency=name):
                    symbol = scopes[function].lookup(name)
                    self.assertEqual(
                        (symbol.is_global(), symbol.is_local(), symbol.is_referenced()),
                        (True, False, True),
                    )

    def test_extracted_modules_keep_imported_and_called_owners_global(self) -> None:
        collisions = []
        for package in ("native_input", "native_capture"):
            for path in (SCRIPTS / package).rglob("*.py"):
                source = path.read_text()
                tree = ast.parse(source)
                compile(source, str(path), "exec")
                table = symtable.symtable(source, str(path), "exec")
                scopes = {child.get_name(): child for child in table.get_children()}
                imports = set()
                helpers = set()
                for node in tree.body:
                    if isinstance(node, ast.Import):
                        imports.update(alias.asname or alias.name.split('.')[0] for alias in node.names)
                    elif isinstance(node, ast.ImportFrom):
                        imports.update(alias.asname or alias.name for alias in node.names)
                    elif isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                        helpers.add(node.name)

                class Dependencies(ast.NodeVisitor):
                    def __init__(self):
                        self.names = set()

                    def generic_visit(self, node):
                        # Nested code owns a separate scope. Future annotations
                        # also do not create executable body references.
                        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef,
                                             ast.ClassDef, ast.Lambda)):
                            return
                        super().generic_visit(node)

                    def visit_AnnAssign(self, node):
                        self.visit(node.target)
                        if node.value is not None:
                            self.visit(node.value)

                    def visit_Call(self, node):
                        if isinstance(node.func, ast.Name) and node.func.id in helpers:
                            self.names.add(node.func.id)
                        self.generic_visit(node)

                    def visit_Attribute(self, node):
                        if isinstance(node.value, ast.Name) and node.value.id in imports:
                            self.names.add(node.value.id)
                        self.generic_visit(node)

                for node in tree.body:
                    if not isinstance(node, ast.FunctionDef):
                        continue
                    uses = Dependencies()
                    for statement in node.body:
                        uses.visit(statement)
                    for name in uses.names:
                        symbol = scopes[node.name].lookup(name)
                        if not symbol.is_global():
                            collisions.append((str(path.relative_to(SCRIPTS)), node.name, name))
        self.assertEqual(collisions, [])


if __name__ == "__main__":
    unittest.main()
