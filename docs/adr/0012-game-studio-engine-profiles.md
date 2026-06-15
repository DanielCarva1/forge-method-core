# ADR 0012: Game Studio Engine Profiles

Forge Game Studio uses one `engine-setup` workflow with compact engine profiles instead of separate Forge workflows or entrypoints for Godot, Unity, Unreal, Phaser, or future engines. Engine profiles preserve engine-specific structure, language, asset, test, and performance assumptions while keeping `$forge-method` routing stable and avoiding a catalog that grows by engine rather than by user intent.
